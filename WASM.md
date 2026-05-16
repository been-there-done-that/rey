# WASM Crypto

The `zoo-wasm` crate compiles the same Rust crypto code used by the desktop client into WebAssembly for the browser. This ensures zero drift between platforms — signup, login, and file encryption use identical algorithms.

## Architecture

```
crates/crypto/          ← Pure Rust crypto (Argon2, X25519, XSalsa20, BLAKE2b, bcrypt)
    └── used by desktop (native) and WASM (browser)

crates/zoo-wasm/        ← WASM bindings exposing crypto + HTTP client
    ├── crypto_wasm.rs  ← Crypto exports (base64-encoded inputs/outputs)
    └── client.rs       ← ZooHandle (HTTP client, optional feature)
```

## Crypto Functions Exposed to Browser

| Function | Purpose |
|----------|---------|
| `generate_key_b64()` | Generate 32-byte random key |
| `generate_keypair_b64()` | Generate X25519 keypair (public + secret) |
| `generate_salt_b64()` | Generate 16-byte random salt |
| `derive_kek_b64(password, salt, mem_limit, ops_limit)` | Argon2id key derivation |
| `derive_verification_key_b64(kek)` | BLAKE2b keyed hash for login verification |
| `bcrypt_hash_b64(plaintext)` | bcrypt hash of verify key |
| `encrypt_key_b64(plaintext, wrapping_key)` | XSalsa20-Poly1305 key wrapping |

## Signup Flow (Browser)

```
1. User enters email + password
2. generate_salt() → random 16-byte salt
3. deriveKek(password, salt) → Argon2id → 32-byte KEK
4. deriveVerificationKey(kek) → BLAKE2b → verify key
5. bcryptHash(verifyKey) → verify_key_hash (sent to server)
6. generateKey() → master key
7. generateKeypair() → X25519 public/secret keypair
8. encryptKey(masterKey, kek) → encrypted_master_key + nonce
9. encryptKey(secretKey, kek) → encrypted_secret_key + nonce
10. encryptKey(recoveryKey, kek) → encrypted_recovery_key + nonce
11. POST /api/auth/register with all 12 fields
```

## Login Flow (Browser)

```
1. User enters email + password
2. POST /api/auth/login-params → { kek_salt, mem_limit, ops_limit }
3. deriveKek(password, kek_salt) → Argon2id → KEK
4. deriveVerificationKey(kek) → BLAKE2b → verify key
5. bcryptHash(verifyKey) → verify_key_hash
6. POST /api/auth/login → { session_token, key_attributes }
7. Store session_token, redirect to /d/home
```

## Building

```bash
# Build WASM module
make wasm

# Clean and rebuild from scratch
make wasm-clean
```

Output goes to `apps/web/src/wasm/` which contains:
- `zoo_wasm.js` — JS glue code (ES module)
- `zoo_wasm_bg.wasm` — compiled WASM binary
- `zoo_wasm.d.ts` — TypeScript declarations
- `index.ts` — re-exports for Next.js import

## WASM Compatibility

All crypto dependencies are pure Rust and WASM-compatible:

| Crate | WASM Support | Notes |
|-------|-------------|-------|
| `argon2` | ✅ | Pure Rust, uses `alloc` feature |
| `x25519-dalek` | ✅ | Pure Rust via curve25519-dalek |
| `chacha20poly1305` | ✅ | Pure Rust |
| `xsalsa20poly1305` | ✅ | Pure Rust (deprecated but functional) |
| `blake2b_simd` | ✅ | Falls back to portable impl on WASM |
| `bcrypt` | ✅ | Pure Rust |
| `getrandom` | ✅ | Uses `wasm_js` feature → `crypto.getRandomValues()` |

## Feature Flags

- `--no-default-features` — builds crypto only (no HTTP client)
- `--features client` — includes `ZooHandle` HTTP client (requires `reqwest` which is not WASM-compatible yet)

The HTTP client feature is disabled for WASM builds because `reqwest` depends on `tokio` which doesn't support `wasm32-unknown-unknown`. The crypto functions work independently and are called from the Next.js frontend, which then uses `fetch`/`ky` for HTTP requests.
