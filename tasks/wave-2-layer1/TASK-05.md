# Task 5: Implement `crates/crypto` — Layer 1 Cryptographic Primitives

## Wave
2 (Layer 1 — Pure Libraries)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types crate) must be complete

## Can Run In Parallel With
- Task 6 (image crate) — no dependencies between crypto and image

## Design References
- design.md §3.1: Key Hierarchy diagram
- design.md §3.2: Crypto Module Structure
- design.md §3.3: Key Types (Key256, Nonce24, Header24, Salt16, EncryptedKey, Argon2Profile)
- design.md §3.4: AEAD Operations (secretbox_encrypt/decrypt, stream_encrypt/decrypt)
- design.md §3.5: KDF Operations (derive_kek, derive_verification_key, derive_subkey)
- design.md §3.6: Argon2id Adaptive Fallback algorithm
- design.md §3.7: Wire Formats (SecretBox, SecretStream, SealedBox)
- SPEC.md §1.1: Key Hierarchy
- SPEC.md §1.2: Algorithms table
- SPEC.md §1.8: Wire Formats

## Requirements
1.1–1.8, 2.4, 3.1–3.6, 4.1–4.4, 5.1–5.8, 6.1–6.3, 25.3

## Objective
Implement ALL cryptographic operations. Zero I/O. Zero platform dependencies. `#![no_std]` compatible. This is the single audit surface for the entire encryption system.

## Cargo.toml
```toml
[package]
name = "crypto"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[features]
default = ["std"]
std = ["rand_core/getrandom", "x25519-dalek/static_secrets"]
no_std = []

[dependencies]
types = { workspace = true }
zeroize = { workspace = true }
# AEAD
chacha20poly1305 = { workspace = true }
xsalsa20poly1305 = { workspace = true }
# Key exchange
x25519-dalek = { workspace = true }
# KDF
argon2 = { workspace = true }
blake2b_simd = { workspace = true }
# Randomness
rand_core = { workspace = true }
# Error handling
thiserror = { workspace = true }
# Encoding (use alloc-compatible versions for no_std)
base64 = { workspace = true }
hex = { workspace = true }
```

## CRITICAL CONSTRAINTS
1. **NO `std::fs`, NO `std::net`, NO `tokio`, NO HTTP, NO DB** — this crate must be pure computation
2. **`#![no_std]` compatible** — when built with `--no-default-features --features no_std`, it must compile without `std`
3. **NEVER return partial plaintext on MAC failure** — if authentication fails, return `CryptoError::MacMismatch` immediately
4. **All keys must be zeroized on drop** — use `ZeroizeOnDrop` derive
5. **Use constant-time comparison** for any key comparison operations

## Files to Create

### `src/lib.rs`
```rust
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod aead;
pub mod kdf;
pub mod key;
pub mod seal;
pub mod error;
pub mod util;

// Re-export key types from types crate for convenience
pub use types::crypto::{Key256, Nonce24, Header24, Salt16, EncryptedKey, Argon2Profile};
```

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("MAC verification failed")]
    MacMismatch,
    #[error("unsupported cipher: {0}")]
    UnsupportedCipher(String),
    #[error("memory allocation failed for Argon2id")]
    AllocationFailed,
    #[error("invalid key length")]
    InvalidKey,
    #[error("invalid nonce length")]
    InvalidNonce,
    #[error("base64 decode error: {0}")]
    Base64Error(String),
    #[error("hex decode error: {0}")]
    HexError(String),
}
```

### `src/aead/mod.rs`
Re-exports:
```rust
pub mod secretbox;
pub mod stream;

pub use secretbox::{secretbox_encrypt, secretbox_decrypt};
pub use stream::{stream_encrypt, stream_decrypt};
```

### `src/aead/secretbox.rs` — XSalsa20-Poly1305 (Key Wrapping)
Implement:
- `secretbox_encrypt(plaintext: &[u8], key: &Key256) -> (Nonce24, Vec<u8>)`:
  - Generate random 24-byte nonce using `OsRng` (or `getrandom` in no_std)
  - Use `xsalsa20poly1305::XSalsa20Poly1305::new(key.0.into())`
  - Encrypt: `cipher.encrypt(nonce.into(), plaintext)`
  - Return `(Nonce24(nonce), ciphertext)` where ciphertext = MAC(16) || ciphertext
  - Wire format: nonce(24) || MAC(16) || ciphertext

- `secretbox_decrypt(nonce: &Nonce24, ciphertext: &[u8], key: &Key256) -> Result<Vec<u8>, CryptoError>`:
  - Use `xsalsa20poly1305::XSalsa20Poly1305::new(key.0.into())`
  - Decrypt: `cipher.decrypt(nonce.0.into(), ciphertext)`
  - On `aead::Error`, return `CryptoError::MacMismatch`
  - NEVER return partial plaintext on failure

### `src/aead/stream.rs` — XChaCha20-Poly1305 (File/Metadata/Thumbnail Data)
Implement:
- `stream_encrypt(plaintext: &[u8], key: &Key256) -> (Header24, Vec<u8>)`:
  - Generate random 24-byte header using `OsRng`
  - Use `chacha20poly1305::XChaCha20Poly1305::new(key.0.into())`
  - Encrypt: `cipher.encrypt(header.into(), plaintext)`
  - Return `(Header24(header), ciphertext)`
  - Wire format: header(24) || ciphertext

- `stream_decrypt(header: &Header24, ciphertext: &[u8], key: &Key256) -> Result<Vec<u8>, CryptoError>`:
  - Use `chacha20poly1305::XChaCha20Poly1305::new(key.0.into())`
  - Decrypt: `header.decrypt(header.0.into(), ciphertext)`
  - On `aead::Error`, return `CryptoError::MacMismatch`
  - Verify Poly1305 MAC before returning ANY bytes

### `src/kdf/mod.rs`
Re-exports:
```rust
pub mod argon;
pub mod blake2b;

pub use argon::derive_kek;
pub use blake2b::{derive_verification_key, derive_subkey};
```

### `src/kdf/argon.rs` — Argon2id (Password → KEK)
Implement `derive_kek(password: &[u8], salt: &Salt16, profile: Argon2Profile) -> Result<Key256, CryptoError>`:
- Use `argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)`
- params: `m_cost = profile.mem_limit() / 1024` (in KB), `t_cost = profile.ops_limit()`, `p_cost = 1`
- **Adaptive fallback loop** (design §3.6):
  ```
  mem = profile.mem_limit()
  ops = profile.ops_limit()
  floor = 32 * 1024 * 1024  // 32 MiB
  loop:
    result = argon2id(password, salt, mem, ops)
    match result:
      Ok(key) → return Key256(key)
      Err(AllocationFailed) if mem > floor:
        mem /= 2; ops *= 2; continue
      Err(AllocationFailed) → return CryptoError::AllocationFailed
      Err(other) → return other
  ```
- Output: 32 bytes → `Key256`

### `src/kdf/blake2b.rs` — BLAKE2b-KDF (Subkey Derivation)
Implement:
- `derive_verification_key(kek: &Key256) -> Key256`:
  - Use `blake2b_simd::Params::new().key(kek.0).personal(b"verification").hash_length(32)`
  - Or use keyed BLAKE2b with context string "verification"
  - Returns `Key256(hash_output)`

- `derive_subkey(master: &Key256, context: &str, id: u64) -> Key256`:
  - Use `blake2b_simd::Params::new().key(master.0).personal(context.as_bytes()[..8]).hash_length(32)`
  - Incorporate `id` into the personalization or as additional data
  - Returns `Key256(hash_output)`

### `src/key/mod.rs`
Re-exports:
```rust
pub mod generate;
pub mod encrypt;
pub mod decrypt;

pub use generate::generate_key;
pub use encrypt::encrypt_key;
pub use decrypt::decrypt_key;
```

### `src/key/generate.rs`
Implement `generate_key() -> Key256`:
- Use `rand_core::OsRng` to fill 32 bytes
- Return `Key256(bytes)`

### `src/key/encrypt.rs`
Implement `encrypt_key(plaintext_key: &Key256, wrapping_key: &Key256) -> EncryptedKey`:
- Call `super::aead::secretbox_encrypt(&plaintext_key.0, wrapping_key)`
- Return `EncryptedKey { nonce, ciphertext }`

### `src/key/decrypt.rs`
Implement `decrypt_key(encrypted: &EncryptedKey, wrapping_key: &Key256) -> Result<Key256, CryptoError>`:
- Call `super::aead::secretbox_decrypt(&encrypted.nonce, &encrypted.ciphertext, wrapping_key)`
- Return `Key256::from_bytes(decrypted)?`

### `src/seal/mod.rs`
Re-exports:
```rust
pub mod keypair;
pub mod box_;

pub use keypair::generate_keypair;
pub use box_::{seal, open};
```

### `src/seal/keypair.rs`
Implement `generate_keypair() -> (x25519_dalek::StaticSecret, x25519_dalek::PublicKey)`:
- Use `x25519_dalek::StaticSecret::random_from_rng(OsRng)`
- Return `(secret, PublicKey::from(&secret))`

### `src/seal/box_.rs` — X25519 Sealed Box (Asymmetric Sharing)
Implement:
- `seal(plaintext: &[u8], recipient_pk: &PublicKey) -> Vec<u8>`:
  - Generate ephemeral keypair: `(ephemeral_sk, ephemeral_pk)`
  - Compute shared secret: `shared = ephemeral_sk.diffie_hellman(recipient_pk)`
  - Derive encryption key from shared secret using BLAKE2b
  - Encrypt with XSalsa20-Poly1305 using derived key
  - Return: `ephemeral_pk(32) || MAC(16) || ciphertext`
  - Wire format: ephemeral_pk(32) || MAC(16) || ciphertext

- `open(ciphertext: &[u8], recipient_sk: &StaticSecret) -> Result<Vec<u8>, CryptoError>`:
  - Extract `ephemeral_pk` from first 32 bytes
  - Compute shared secret: `shared = recipient_sk.diffie_hellman(&ephemeral_pk)`
  - Derive same encryption key from shared secret
  - Decrypt remaining bytes with XSalsa20-Poly1305
  - On MAC failure, return `CryptoError::MacMismatch`

### `src/util.rs`
Implement:
- `constant_time_eq(a: &[u8], b: &[u8]) -> bool` — use `subtle::ConstantTimeEq` or implement manually
- `zeroize_bytes(bytes: &mut [u8])` — overwrite with zeros
- `base64_encode(data: &[u8]) -> String` — using `base64::Engine`
- `base64_decode(s: &str) -> Result<Vec<u8>, CryptoError>`
- `hex_encode(data: &[u8]) -> String`
- `hex_decode(s: &str) -> Result<Vec<u8>, CryptoError>`

## Tests

### Property Tests (Tasks 5.10–5.13 — marked with *)
Use `proptest` crate:
- **Property 1**: `∀ key ∈ Key256, plaintext ∈ Vec<u8>: stream_decrypt(stream_encrypt(plaintext, key)) == Ok(plaintext)`
- **Property 2**: `∀ key ∈ Key256, plaintext ∈ Vec<u8>: secretbox_decrypt(secretbox_encrypt(plaintext, key)) == Ok(plaintext)`
- **Property 3**: `∀ key ∈ Key256, ciphertext ∈ Vec<u8> (with one byte flipped): stream_decrypt(ciphertext, key) == Err(MacMismatch)` — verify no partial plaintext
- **Property 4**: `∀ recipient_keypair, plaintext ∈ Vec<u8>: open(seal(plaintext, pk), sk) == Ok(plaintext)`

### Unit Tests (Task 5.14 — marked with *)
- Argon2id Known Answer Test against libsodium reference vectors
- Adaptive fallback: mock allocation failure, verify mem halves and ops doubles
- Fallback stops at 32 MiB floor and returns `AllocationFailed`
- BLAKE2b-KDF VerificationKey derivation matches reference output
- `generate_key()` produces 32 bytes, multiple calls produce different keys
- `constant_time_eq` returns true for equal slices, false for different

### no_std Verification (Task 5.15)
- `cargo build -p crypto --no-default-features --features no_std` must succeed
- `cargo tree -p crypto --no-default-features --features no_std` must show no std-dependent crates

## Verification Steps
- [ ] `cargo check -p crypto` succeeds
- [ ] `cargo test -p crypto` passes all property tests and unit tests
- [ ] `cargo build -p crypto --no-default-features --features no_std` succeeds
- [ ] `cargo clippy -p crypto -- -D warnings` is clean
- [ ] No `std::fs`, `std::net`, `tokio`, or HTTP imports anywhere
- [ ] All key types implement `ZeroizeOnDrop`
- [ ] `secretbox_decrypt` and `stream_decrypt` return `MacMismatch` on tampered ciphertext (no partial plaintext)
- [ ] Wire format: SecretBox = nonce(24) || MAC(16) || ciphertext
- [ ] Wire format: SecretStream = header(24) || ciphertext
- [ ] Wire format: SealedBox = ephemeral_pk(32) || MAC(16) || ciphertext
- [ ] Argon2id fallback loop: 256 MiB → 128 MiB → 64 MiB → 32 MiB floor

## Notes
- This is the most security-critical crate. Every function must be correct.
- Use the `aead` crate's `Aead` trait for XSalsa20-Poly1305 and XChaCha20-Poly1305.
- The `chacha20poly1305` crate provides both `ChaCha20Poly1305` and `XChaCha20Poly1305`.
- The `xsalsa20poly1305` crate provides `XSalsa20Poly1305`.
- For `no_std`, use `getrandom` crate with `js` feature for WASM, or `rdrand` for x86.
- The `subtle` crate provides constant-time comparison (`ConstantTimeEq`).
- `Key256` must NOT implement `PartialEq` or `Eq` to prevent accidental key comparison.
- For the sealed box, the `x25519-dalek` crate's `StaticSecret::diffie_hellman` computes the shared secret.
