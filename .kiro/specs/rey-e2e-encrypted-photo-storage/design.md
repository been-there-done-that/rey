# Rey — Technical Design Document

> **Scope**: This document translates every requirement in `requirements.md` into a concrete implementation plan. It is grounded in `SPEC.md`, `ARCHITECTURE.md`, `STRUCTURE.md`, and `ZOO.md`. Where those documents contain known issues (catalogued in `NITPICKS.md`), this design incorporates the fixes.

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Components and Interfaces](#components-and-interfaces)
4. [Data Models](#data-models)
5. [Error Handling](#error-handling)
6. [Correctness Properties](#correctness-properties)
7. [Testing Strategy](#testing-strategy)

---

## Overview

Rey is an end-to-end encrypted photo storage application. All encryption and decryption execute exclusively on the user's device; the Zoo server stores only ciphertext and coordinates access. No plaintext, no keys, and no decrypted metadata ever cross the network boundary.

This design document covers the full implementation of the system as described in requirements 1–28:

- **Authentication & Key Management** (Req 1–3): Argon2id-based key derivation, MasterKey hierarchy, VerificationKey login protocol.
- **File & Collection Encryption** (Req 4–6): Per-collection and per-file key hierarchy, XChaCha20-Poly1305 file encryption, X25519 sealed-box sharing.
- **Sync Engine** (Req 7–8): Incremental diff sync with version-consistent pagination and cursor persistence.
- **Local Database** (Req 9, 21): SQLite with FTS5 full-text search, encrypted at rest.
- **Thumbnail Pipeline** (Req 10–11): Client-side generation, encryption, two-level LRU cache.
- **Upload Service** (Req 12–19, 27–28): Zoo state machine, multipart presigning, stall detection, GC, SSE hub.
- **File Download** (Req 20): Redirect and proxy modes.
- **Platform Support** (Req 22–23): Tauri desktop and WASM web.
- **Zero-Knowledge Guarantee** (Req 24): Compile-time enforcement via Cargo dependency graph.
- **Crate Architecture** (Req 25): Virtual workspace, layered DAG, import rules.
- **EXIF Extraction** (Req 26): GPS, datetime, orientation from image metadata.

---

## Architecture

### 2.1 Cargo Workspace Layout

The workspace root `Cargo.toml` is a **virtual manifest** (no `[package]` section). All crates live under `crates/`. This enforces explicit `-p` flags and prevents accidental root-level compilation.

```
Cargo.toml                    # Virtual workspace manifest
crates/
├── types/          Layer 0   ships: client + server
├── common/         Layer 0   ships: client + server
├── crypto/         Layer 1   ships: client only
├── image/          Layer 1   ships: client only
├── metadata/       Layer 1   ships: client only
├── thumbnail/      Layer 1   ships: client only
├── local-db/       Layer 2   ships: client only
├── sync/           Layer 2   ships: client only
├── zoo-client/     Layer 2   ships: client only
├── zoo/            Layer 2   ships: server only
├── client-lib/     Layer 3   ships: desktop (Tauri)
└── zoo-wasm/       Layer 3   ships: web (WASM)
```

**Layer definitions:**

| Layer | Crates | Constraint |
|-------|--------|------------|
| 0 — Foundation | `types`, `common` | No internal deps between them. Used by everything. |
| 1 — Pure Libraries | `crypto`, `image`, `metadata`, `thumbnail` | Zero framework coupling. No HTTP. No DB. No Tauri. |
| 2 — Application Logic | `sync`, `local-db`, `zoo-client`, `zoo` | Client and server separated at compile time. |
| 3 — Platform Bindings | `client-lib`, `zoo-wasm` | No business logic. Only platform wrappers. |

### 2.2 Dependency Graph

```
                    ┌─────────────┐   ┌─────────────┐
                    │  client-lib │   │  zoo-wasm   │
                    │  (Layer 3)  │   │  (Layer 3)  │
                    └──────┬──────┘   └──────┬──────┘
                           │                 │
              ┌────────────┼──────┐          │
              │            │      │          │
              ▼            ▼      ▼          ▼
           ┌──────┐  ┌──────┐  ┌──────┐  ┌────────────┐
           │ sync │  │local │  │thumb │  │ zoo-client │
           │      │  │ -db  │  │ nail │  │  (Layer 2) │
           └──┬───┘  └──┬───┘  └──┬───┘  └────────────┘
              │         │         │
              └────┬────┘         │
                   │              │
              ┌────▼────┐    ┌────▼────┐
              │metadata │    │  image  │
              └────┬────┘    └────┬────┘
                   │              │
              ┌────▼──────────────▼────┐
              │         crypto         │
              │        (Layer 1)       │
              └────────────┬───────────┘
                           │
              ┌────────────▼───────────┐
              │          types         │
              │        (Layer 0)       │
              └────────────┬───────────┘
                           │
              ┌────────────▼───────────┐
              │          common        │
              │        (Layer 0)       │
              └────────────────────────┘

              ┌────────────────────────┐
              │           zoo          │
              │        (Layer 2)       │
              │  depends: types,common │
              │  (server-only island)  │
              └────────────────────────┘
```

`zoo` is a fully independent island — it shares only `types` and `common` with the client graph. It never imports `crypto`, `image`, `metadata`, `thumbnail`, `sync`, `local-db`, or `client-lib`.

### 2.3 Compilation Guarantees

These violations are prevented at compile time by Cargo's dependency resolver. If any `Cargo.toml` were to add a forbidden dependency, `cargo build` would fail with a dependency resolution error (Req 25.7).

| Violation | Prevention mechanism |
|-----------|---------------------|
| `zoo` imports `crypto` | `crypto` not listed in `zoo/Cargo.toml` |
| `zoo` imports `image` | `image` not listed in `zoo/Cargo.toml` |
| `zoo` imports `sync` | `sync` not listed in `zoo/Cargo.toml` |
| `zoo` imports `local-db` | `local-db` not listed in `zoo/Cargo.toml` |
| `zoo` imports `client-lib` | `client-lib` not listed in `zoo/Cargo.toml` |
| `client-lib` imports `axum` | `axum` not listed in `client-lib/Cargo.toml` |
| `client-lib` imports `sqlx` (postgres) | `sqlx` postgres feature not in `client-lib/Cargo.toml` |
| `zoo-wasm` imports `crypto` | `crypto` not listed in `zoo-wasm/Cargo.toml` |
| `zoo-wasm` imports `image` | `image` not listed in `zoo-wasm/Cargo.toml` |
| `image` imports `crypto` | `crypto` not listed in `image/Cargo.toml` |
| `crypto` imports any I/O crate | `#![no_std]` compatible; no `std::fs`, no `tokio` |
| `types` imports any internal crate | Only `serde`, `serde_json` in `types/Cargo.toml` |

### 2.4 Feature Flags

Implements STRUCTURE.md §8. Feature flags gate platform-specific code and optional backends.

| Crate | Feature | Effect | Default |
|---|---|---|---|
| `crypto` | `std` | Full std crypto (rand, OsRng) | Yes |
| `crypto` | `no_std` | Disable std, for embedded/WASM targets | No |
| `zoo` | `s3` | AWS S3 storage backend | Yes |
| `zoo` | `local-fs` | Local filesystem storage backend (dev/testing) | No |
| `client-lib` | `desktop` | Enable Tauri desktop commands | Yes |
| `local-db` | `sqlcipher` | SQLCipher encryption for SQLite | Yes |

**Usage:** `cargo build -p crypto --no-default-features --features no_std` for WASM targets. `cargo build -p zoo --no-default-features --features local-fs` for local dev without S3.

### 2.5 Build Order & Compilation Parallelism

Implements STRUCTURE.md §7. Cargo compiles independent crates in parallel at each step.

```
Step 1 (2 in parallel):  types, common

Step 2 (4 in parallel):  crypto, image, zoo-client, zoo

Step 3 (2 in parallel):  metadata, thumbnail

Step 4 (2 in parallel):  local-db, sync

Step 5 (2 in parallel):  client-lib, zoo-wasm
```

**Change Impact:**

| Change to | Recompiles |
|---|---|
| `types` | Everything |
| `crypto` | `metadata`, `thumbnail`, `sync`, `client-lib` |
| `image` | `metadata`, `thumbnail`, `sync`, `client-lib` |
| `zoo` | Nothing outside server (self-contained) |
| `sync` | `client-lib` only |
| `client-lib` | Nothing downstream (terminal crate) |


---

## Components and Interfaces

This section covers all crates and their interfaces. Sub-sections map to individual crates.

### Crypto Module (`crates/crypto`)

Implements Req 1–6, Req 25.3. All cryptography. Zero I/O. Zero platform dependencies. `#![no_std]` compatible.

### 3.1 Key Hierarchy

```
Password + Salt
     │
     ▼ Argon2id v1.3 (Sensitive: 256 MiB / 4 ops)
     │
  KEK (Key Encryption Key, 256-bit, never stored)
     │
     ├──────────────────────────────────────────────┐
     │                                              │
     ▼ XSalsa20-Poly1305 (SecretBox)                ▼ BLAKE2b-KDF
     │  encrypts MasterKey                          │  context="verification", id=2
     │  → EncryptedMasterKey + Nonce24              │
     │                                              ▼
     ▼                                        VerificationKey (256-bit)
  MasterKey (256-bit, in secret memory)             │
     │                                              ▼ SHA-256
     │                                        VerifyKeyHash
     │                                        (transmitted to server)
     │                                        (server stores bcrypt(VerifyKeyHash))
     │
     ├── XSalsa20-Poly1305 → EncryptedCollectionKey + Nonce24
     │       (one per collection)
     │
     ├── XSalsa20-Poly1305 → EncryptedSecretKey + Nonce24
     │       (X25519 private key for sharing)
     │
     └── XSalsa20-Poly1305 → EncryptedRecoveryKey + Nonce24

CollectionKey (256-bit, per album)
     │
     └── XSalsa20-Poly1305 → EncryptedFileKey + Nonce24
             (one per file)

FileKey (256-bit, per file)
     │
     ├── XChaCha20-Poly1305 (SecretStream) → encrypted file data + Header24
     ├── XChaCha20-Poly1305 (SecretStream) → encrypted metadata + Header24
     └── XChaCha20-Poly1305 (SecretStream) → encrypted thumbnail + Header24
```

### 3.2 Module Structure

```
crates/crypto/
├── Cargo.toml            # deps: types, aead, xchacha20poly1305,
│                         #       xsalsa20poly1305, x25519-dalek, argon2,
│                         #       blake2b_simd, rand_core
│                         # #![no_std] compatible
└── src/
    ├── lib.rs            # Crate root; re-exports public API; #![no_std]
    ├── error.rs          # CryptoError enum: MacMismatch, UnsupportedCipher,
    │                     #   AllocationFailed, InvalidKey, InvalidNonce
    ├── types.rs          # Key256, Nonce24, Salt16, Header24, EncryptedKey,
    │                     #   KeyAttributes, Argon2Profile (re-exported from types)
    ├── aead/
    │   ├── mod.rs        # Re-exports: secretbox_encrypt/decrypt, stream_encrypt/decrypt
    │   ├── secretbox.rs  # XSalsa20-Poly1305: key wrapping (KEK→MasterKey, etc.)
    │   └── stream.rs     # XChaCha20-Poly1305: file/thumbnail/metadata data
    ├── kdf/
    │   ├── mod.rs        # Re-exports: derive_kek, derive_verification_key, derive_subkey
    │   ├── argon.rs      # Argon2id v1.3: password → KEK; adaptive fallback loop
    │   └── blake2b.rs    # BLAKE2b-KDF: MasterKey → subkeys (VerificationKey, etc.)
    ├── key/
    │   ├── mod.rs        # Re-exports: generate_key, encrypt_key, decrypt_key
    │   ├── generate.rs   # OsRng 256-bit key generation
    │   ├── encrypt.rs    # Encrypt a key with another key (SecretBox)
    │   └── decrypt.rs    # Decrypt a key (SecretBox open)
    ├── seal/
    │   ├── mod.rs        # Re-exports: seal, open, generate_keypair
    │   ├── box_.rs       # X25519 sealed box: ephemeral keypair + XSalsa20-Poly1305
    │   └── keypair.rs    # X25519 keypair generation via OsRng
    └── util.rs           # constant_time_eq, zeroize helpers, base64/hex encoding
```

### 3.3 Key Types

```rust
/// A 256-bit symmetric key. Zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Key256([u8; 32]);

/// A 24-byte nonce for XSalsa20-Poly1305 (SecretBox).
#[derive(Clone, Copy)]
pub struct Nonce24([u8; 24]);

/// A 24-byte header for XChaCha20-Poly1305 (SecretStream).
#[derive(Clone, Copy)]
pub struct Header24([u8; 24]);

/// A 16-byte random salt for Argon2id.
#[derive(Clone, Copy)]
pub struct Salt16([u8; 16]);

/// An encrypted key blob: nonce || MAC || ciphertext.
/// Used for all SecretBox-encrypted keys (MasterKey, CollectionKey, FileKey, etc.)
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedKey {
    pub nonce: Nonce24,
    pub ciphertext: Vec<u8>,  // MAC (16 bytes) || ciphertext
}

/// Key derivation parameters stored on the server and returned at login.
/// Allows the client to re-derive the KEK on any device.
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyAttributes {
    pub encrypted_master_key: EncryptedKey,  // XSalsa20-Poly1305(KEK, MasterKey)
    pub kek_salt: Salt16,                    // Argon2id salt
    pub mem_limit: u32,                      // Argon2id memory in bytes
    pub ops_limit: u32,                      // Argon2id iterations
}

/// Argon2id parameter profiles (Req 3.5, 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argon2Profile {
    /// 256 MiB / 4 ops — first unlock on desktop (Req 1.1)
    Sensitive,
    /// 128 MiB / 3 ops — first unlock on mobile (Req 3.5)
    Mobile,
    /// 64 MiB / 2 ops — quick re-auth (Req 3.6)
    Interactive,
}

impl Argon2Profile {
    pub fn mem_limit(&self) -> u32 {
        match self {
            Self::Sensitive    => 256 * 1024 * 1024,
            Self::Mobile       => 128 * 1024 * 1024,
            Self::Interactive  =>  64 * 1024 * 1024,
        }
    }
    pub fn ops_limit(&self) -> u32 {
        match self { Self::Sensitive => 4, Self::Mobile => 3, Self::Interactive => 2 }
    }
}
```

### 3.4 AEAD Operations

```rust
// crates/crypto/src/aead/secretbox.rs

/// Encrypt `plaintext` with `key` using XSalsa20-Poly1305.
/// Returns a random 24-byte nonce and the ciphertext (MAC || ciphertext).
/// Wire format: nonce (24) || MAC (16) || ciphertext.
/// Used for key wrapping: KEK→MasterKey, MasterKey→CollectionKey, etc. (Req 1.3, 4.2, 4.4)
pub fn secretbox_encrypt(plaintext: &[u8], key: &Key256) -> (Nonce24, Vec<u8>);

/// Decrypt `ciphertext` (MAC || ciphertext) with `key` and `nonce`.
/// Returns Err(CryptoError::MacMismatch) if authentication fails.
/// NEVER returns partial plaintext on failure (Req 5.7).
pub fn secretbox_decrypt(
    nonce: &Nonce24,
    ciphertext: &[u8],
    key: &Key256,
) -> Result<Vec<u8>, CryptoError>;

// crates/crypto/src/aead/stream.rs

/// Encrypt `plaintext` with `key` using XChaCha20-Poly1305 (SecretStream).
/// Returns a random 24-byte header and the ciphertext.
/// Wire format: header (24) || ciphertext.
/// Used for file data, metadata, and thumbnail encryption (Req 5.1, 5.2, 5.3).
pub fn stream_encrypt(plaintext: &[u8], key: &Key256) -> (Header24, Vec<u8>);

/// Decrypt `ciphertext` with `key` and `header`.
/// Verifies the Poly1305 MAC before returning any plaintext (Req 5.6).
/// Returns Err(CryptoError::MacMismatch) if verification fails.
/// NEVER returns partial plaintext on failure (Req 5.7).
pub fn stream_decrypt(
    header: &Header24,
    ciphertext: &[u8],
    key: &Key256,
) -> Result<Vec<u8>, CryptoError>;
```

### 3.5 KDF Operations

```rust
// crates/crypto/src/kdf/argon.rs

/// Derive a KEK from `password` and `salt` using Argon2id v1.3.
/// Applies the adaptive fallback loop (Req 3.1–3.4).
/// Returns Err(CryptoError::AllocationFailed) if even 32 MiB fails.
pub fn derive_kek(
    password: &[u8],
    salt: &Salt16,
    profile: Argon2Profile,
) -> Result<Key256, CryptoError>;

// crates/crypto/src/kdf/blake2b.rs

/// Derive the VerificationKey from the KEK using BLAKE2b-KDF.
/// context = "verification", subkey_id = 2 (Req 1.4, 2.3).
/// Used to produce VerifyKeyHash = SHA-256(VerificationKey).
pub fn derive_verification_key(kek: &Key256) -> Key256;

/// Derive a subkey from `master` using BLAKE2b-KDF.
/// `context` is an 8-byte ASCII string (e.g., "filekey_").
/// `id` is the subkey index (e.g., collection_id hash).
/// Used for future subkey derivation needs (Req 4, cipher agility).
pub fn derive_subkey(master: &Key256, context: &str, id: u64) -> Key256;
```

### 3.6 Argon2id Adaptive Fallback

Implements Req 3.1–3.4. The loop halves memory and doubles ops on each allocation failure, down to a 32 MiB floor.

```
fn derive_kek(password, salt, profile) -> Result<Key256, CryptoError>:
    mem  = profile.mem_limit()   // e.g. 256 MiB for Sensitive
    ops  = profile.ops_limit()   // e.g. 4 for Sensitive
    floor = 32 * 1024 * 1024     // 32 MiB absolute floor

    loop:
        result = argon2id_v13(password, salt, mem, ops, output_len=32)

        match result:
            Ok(key)  → return Ok(Key256(key))
            Err(AllocationFailed) if mem > floor:
                mem  = mem / 2
                ops  = ops * 2
                continue
            Err(AllocationFailed):
                // mem == floor and still failing
                return Err(CryptoError::AllocationFailed)
            Err(other) → return Err(other)
```

**Note**: `ops` doubling compensates for reduced memory to maintain equivalent work factor. The 32 MiB floor (Req 3.4) prevents infinite loops on severely constrained devices.

### 3.7 Wire Formats

All formats are defined in Req 5.4, 5.5, 6.2 and SPEC.md §1.8.

**SecretBox (XSalsa20-Poly1305) — key wrapping:**
```
┌─────────────────────────────────────────────────────────────┐
│  nonce (24 bytes)  │  MAC (16 bytes)  │  ciphertext (N bytes) │
└─────────────────────────────────────────────────────────────┘
Total: 40 + N bytes
Used for: EncryptedMasterKey, EncryptedCollectionKey, EncryptedFileKey,
          EncryptedSecretKey, EncryptedRecoveryKey
```

**SecretStream (XChaCha20-Poly1305) — file/metadata/thumbnail data:**
```
┌──────────────────────────────────────────────────────────────┐
│  header (24 bytes)  │  ciphertext (N + 17 bytes per chunk)   │
└──────────────────────────────────────────────────────────────┘
Total: 24 + N + overhead bytes
Used for: encrypted file data, encrypted metadata, encrypted thumbnail
```

**SealedBox (X25519 + XSalsa20-Poly1305) — asymmetric sharing:**
```
┌──────────────────────────────────────────────────────────────────┐
│  ephemeral_pk (32 bytes)  │  MAC (16 bytes)  │  ciphertext (N bytes) │
└──────────────────────────────────────────────────────────────────┘
Total: 48 + N bytes
Used for: sharing CollectionKey with recipient's PublicKey (Req 6.1, 6.2)
```


---

## Data Models

This section defines all shared data types in `crates/types` and the database schemas.

### Types Crate (`crates/types`)

Implements Req 25.2. Pure data types with `serde` derives. Zero logic. Ships in both client and server.

### 4.1 Module Structure

```
crates/types/
├── Cargo.toml            # deps: serde, serde_json only (Req 25.2)
└── src/
    ├── lib.rs            # Re-exports all public types
    ├── crypto.rs         # Key256, Nonce24, Header24, Salt16, EncryptedKey,
    │                     #   KeyAttributes, Argon2Profile
    ├── file.rs           # FileMetadata, FileRecord, EncryptedFileRecord
    ├── collection.rs     # Collection, EncryptedCollection
    ├── sync.rs           # SyncCollectionResponse, SyncFilesResponse,
    │                     #   SyncTrashResponse, SyncCursor
    ├── upload.rs         # UploadStatus, UploadState, PartRecord, UploadSummary
    ├── sse.rs            # SseEvent enum (all event variants)
    ├── device.rs         # DeviceInfo, DeviceRegistration, DevicePlatform
    ├── share.rs          # ShareRecord, ShareRequest
    ├── user.rs           # UserRegistration, LoginParams, LoginRequest,
    │                     #   LoginResponse, SessionInfo
    └── error.rs          # ErrorCode, ErrorResponse, ApiError
```

### 4.2 Core Types

```rust
// crates/types/src/file.rs

/// Decrypted file metadata — stored in Local_DB after sync (Req 9.2).
/// Encrypted as a JSON blob with the FileKey before transmission (Req 5.2, 26.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,       // Unix timestamp (milliseconds)
    pub device_make: Option<String>,
    pub device_model: Option<String>,
    pub tags: Vec<String>,
}

/// Encrypted file record as stored on the server and returned by sync (Req 8.2).
/// All sensitive fields are ciphertext; the server never sees plaintext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFileRecord {
    pub id: i64,
    pub collection_id: String,
    /// Cipher identifier for agility (Req 4.7, 5). Default: "xchacha20-poly1305".
    pub cipher: String,
    /// FileKey encrypted with CollectionKey (XSalsa20-Poly1305).
    pub encrypted_key: String,          // base64(nonce || MAC || ciphertext)
    pub key_decryption_nonce: String,   // base64(24-byte nonce)
    /// XChaCha20-Poly1305 header for file data decryption.
    pub file_decryption_header: String, // base64(24-byte header)
    /// XChaCha20-Poly1305 header for thumbnail decryption (optional).
    pub thumb_decryption_header: Option<String>,
    /// Encrypted FileMetadata JSON (XChaCha20-Poly1305 with FileKey).
    pub encrypted_metadata: String,
    /// Encrypted thumbnail bytes (XChaCha20-Poly1305 with FileKey, optional).
    pub encrypted_thumbnail: Option<String>,
    pub thumbnail_size: Option<i32>,
    pub file_size: i64,
    pub mime_type: String,
    /// SHA-256 of plaintext — known metadata leak (Req 24.6, SPEC §5.1).
    pub content_hash: String,
    pub object_key: String,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

// crates/types/src/crypto.rs

/// Key derivation parameters returned by the server at login (Req 2.5).
/// Allows the client to re-derive the KEK on any device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyAttributes {
    /// MasterKey encrypted with KEK (XSalsa20-Poly1305).
    pub encrypted_master_key: String,   // base64(nonce || MAC || ciphertext)
    pub key_nonce: String,              // base64(24-byte nonce)
    pub kek_salt: String,               // base64(16-byte salt)
    pub mem_limit: u32,                 // Argon2id memory in bytes
    pub ops_limit: u32,                 // Argon2id iterations
}

// crates/types/src/upload.rs

/// Upload lifecycle states (Req 13.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    Pending,
    Encrypting,
    Uploading,
    S3Completed,
    Registering,
    Done,
    Stalled,
    Failed,
}

// crates/types/src/sse.rs

/// All SSE event variants broadcast by the Zoo SSE Hub (Req 19.4, 19.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    UploadProgress {
        upload_id: String,
        status: UploadStatus,
        parts_bitmask: String,  // base64-encoded big-endian bit vector
        part_count: u16,
        device_name: String,
    },
    UploadCompleted {
        upload_id: String,
        device_name: String,
    },
    UploadDone {
        upload_id: String,
        file_id: i64,
        device_name: String,
    },
    UploadStalled {
        upload_id: String,
        parts_bitmask: String,
        part_count: u16,
        device_name: String,
        stalled_at: i64,
    },
    UploadFailed {
        upload_id: String,
        reason: String,
        device_name: String,
    },
    UploadPending {
        uploads: Vec<UploadSummary>,
    },
    DeviceConnected {
        device_id: String,
        device_name: String,
    },
    DeviceDisconnected {
        device_id: String,
        device_name: String,
    },
    Heartbeat {
        timestamp: i64,
    },
}

// crates/types/src/sync.rs

/// Response from GET /api/sync/collections (Req 7.2, 7.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCollectionResponse {
    pub collections: Vec<EncryptedCollection>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

/// Response from GET /api/sync/files (Req 8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFilesResponse {
    pub updated_files: Vec<EncryptedFileRecord>,
    pub deleted_file_ids: Vec<i64>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

/// Response from GET /api/sync/trash (Req 8.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTrashResponse {
    pub deleted_files: Vec<DeletedFileRef>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFileRef {
    pub file_id: i64,
    pub collection_id: String,
    pub updation_time: i64,
}
```


---

### Zoo Server (`crates/zoo`)

Implements Req 1.9–1.11, 2.1–2.10, 6.4–6.5, 12–19, 20, 24, 27–28.

### 5.1 Module Structure

```
crates/zoo/
├── Cargo.toml              # deps: types, common, axum, tokio, sqlx (postgres),
│                           #   aws-sdk-s3, tower, serde, bcrypt, uuid, utoipa
│                           # NO crypto, image, metadata, thumbnail (Req 24.3)
├── src/
│   ├── lib.rs              # Crate root; module declarations
│   ├── config.rs           # ZooConfig: env-driven configuration
│   ├── types.rs            # Server-specific protocol types (UploadState, etc.)
│   ├── state.rs            # Upload state machine transitions and validation
│   ├── db/
│   │   ├── mod.rs          # DB pool initialization, migration runner
│   │   ├── models.rs       # DB row structs (UploadRow, FileRow, DeviceRow, etc.)
│   │   ├── users.rs        # User CRUD: register, lookup by email, store bcrypt hash
│   │   ├── sessions.rs     # Session token CRUD: store SHA-256 hash, lookup, revoke
│   │   ├── devices.rs      # Device CRUD: register, tombstone, lookup by sse_token
│   │   ├── uploads.rs      # Upload CRUD: create, patch status, bitmask, heartbeat
│   │   ├── upload_parts.rs # Part CRUD: insert batch, mark uploaded, list pending
│   │   └── files.rs        # File CRUD: insert on register, lookup for download
│   ├── s3/
│   │   ├── mod.rs          # S3 client initialization
│   │   ├── presigner.rs    # Presigned PUT/GET URL generation
│   │   └── client.rs       # HeadObject, ListParts, AbortMultipartUpload, DeleteObject
│   ├── sse/
│   │   ├── mod.rs          # SseHub initialization, Postgres LISTEN/NOTIFY setup
│   │   ├── hub.rs          # Per-user broadcast::Sender map (Req 19.6)
│   │   └── events.rs       # SseEvent serialization to text/event-stream format
│   ├── workers/
│   │   ├── mod.rs          # Worker task spawning
│   │   ├── stall_detector.rs  # 15s loop: UPLOADING → STALLED (Req 16)
│   │   └── garbage_collector.rs # 5m loop: expired uploads → FAILED + S3 cleanup (Req 18)
│   ├── api/
│   │   ├── mod.rs          # Axum router construction; utoipa OpenAPI spec generation
│   │   ├── auth.rs         # POST /api/auth/params, POST /api/auth/login, DELETE /api/auth/logout
│   │   ├── devices.rs      # POST /api/devices, PATCH /api/devices/me, DELETE /api/devices/me
│   │   ├── uploads.rs      # Full upload lifecycle endpoints
│   │   ├── files.rs        # GET /api/files/{id}/download, GET /api/files/{id}/thumbnail
│   │   ├── sync.rs         # GET /api/sync/collections, /files, /trash
│   │   └── events.rs       # GET /api/events (SSE stream)
│   └── auth/
│       ├── mod.rs          # Auth middleware: extract session token, hash, lookup user_id
│       └── middleware.rs   # Tower layer for session token validation
├── bin/
│   └── zoo-server.rs       # Binary entrypoint: load config, init DB, start server
└── migrations/
    ├── 001_create_users.sql
    ├── 002_create_sessions.sql
    ├── 003_create_devices.sql
    ├── 004_create_uploads.sql
    ├── 005_create_upload_parts.sql
    ├── 006_create_files.sql
    └── 007_create_shares.sql
```

### 5.2 Database Schema

Full PostgreSQL DDL. Fixes NITPICKS.md #44: `parts_bitmask` encoding is explicitly defined as a big-endian bit vector where bit N (0-indexed from MSB of byte 0) represents part N. Fixes NITPICKS.md #54: dedup index includes `collection_id`.

```sql
-- 001_create_users.sql
CREATE TABLE users (
    user_id         TEXT PRIMARY KEY,           -- UUID as text
    email           TEXT NOT NULL UNIQUE,
    -- bcrypt(SHA-256(VerificationKey)) — never plaintext (Req 1.10, 2.4)
    verify_key_hash TEXT NOT NULL,
    -- Argon2id params for KEK re-derivation (Req 2.1)
    kek_salt        TEXT NOT NULL,              -- base64(16-byte salt)
    mem_limit       INTEGER NOT NULL,           -- bytes
    ops_limit       INTEGER NOT NULL,           -- iterations
    -- MasterKey encrypted with KEK (Req 1.3, 2.5)
    encrypted_master_key TEXT NOT NULL,         -- base64(nonce || MAC || ciphertext)
    key_nonce            TEXT NOT NULL,         -- base64(24-byte nonce)
    -- X25519 public key for sharing (Req 1.6)
    public_key           TEXT NOT NULL,         -- base64(32-byte X25519 public key)
    -- X25519 private key encrypted with MasterKey (Req 1.7)
    encrypted_secret_key TEXT NOT NULL,
    secret_key_nonce     TEXT NOT NULL,
    -- RecoveryKey encrypted with MasterKey (Req 1.8)
    encrypted_recovery_key TEXT NOT NULL,
    recovery_key_nonce     TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 002_create_sessions.sql
CREATE TABLE sessions (
    -- SHA-256(SessionToken) — never the raw token (Req 2.5, 2.8)
    token_hash  TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ NOT NULL               -- default: NOW() + 30 days
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

-- 003_create_devices.sql
CREATE TABLE devices (
    device_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    platform        TEXT NOT NULL CHECK (platform IN ('android','ios','web','desktop')),
    sse_token       TEXT NOT NULL UNIQUE,
    push_token      TEXT,
    -- Per-device stall sensitivity (Req 16.5)
    stall_timeout_seconds INTEGER NOT NULL DEFAULT 90,
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at     TIMESTAMPTZ                     -- tombstone on device removal (Req 12.6)
);

CREATE UNIQUE INDEX idx_devices_user_name ON devices(user_id, name)
    WHERE archived_at IS NULL;
CREATE INDEX idx_devices_sse_token ON devices(sse_token);
CREATE INDEX idx_devices_user_id ON devices(user_id);

-- 004_create_uploads.sql
CREATE TABLE uploads (
    upload_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    device_id           UUID NOT NULL REFERENCES devices(device_id),
    status              TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN (
                                'pending','encrypting','uploading',
                                's3_completed','registering','done',
                                'stalled','failed'
                            )),
    file_hash           TEXT NOT NULL,
    file_size           BIGINT NOT NULL,
    mime_type           TEXT,
    part_size           INTEGER NOT NULL,
    part_count          SMALLINT NOT NULL,
    -- Big-endian bit vector: bit N (0-indexed) = part N uploaded (1) or pending (0).
    -- Byte 0 bit 7 = part 0, byte 0 bit 6 = part 1, ..., byte 1 bit 7 = part 8, etc.
    -- Fixes NITPICKS.md #44: encoding is explicitly defined.
    parts_bitmask       BYTEA NOT NULL DEFAULT '\x'::BYTEA,
    object_key          TEXT,
    upload_id_s3        TEXT,
    complete_url        TEXT,
    urls_expire_at      TIMESTAMPTZ,
    encrypting_at       TIMESTAMPTZ,
    uploading_at        TIMESTAMPTZ,
    last_heartbeat_at   TIMESTAMPTZ,
    stalled_at          TIMESTAMPTZ,
    error_reason        TEXT,
    metadata            JSONB,
    payload             JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMPTZ NOT NULL,
    done_at             TIMESTAMPTZ
);

CREATE INDEX idx_uploads_user_status ON uploads(user_id, status);
-- Dedup: prevent duplicate active uploads for same file in same collection (Req 12.5, NITPICKS #54)
CREATE UNIQUE INDEX idx_uploads_active_dedup
    ON uploads(user_id, file_hash, (metadata->>'collection_id'))
    WHERE status IN ('pending','encrypting','uploading');
CREATE INDEX idx_uploads_heartbeat ON uploads(last_heartbeat_at)
    WHERE status = 'uploading';
CREATE INDEX idx_uploads_expires ON uploads(expires_at)
    WHERE status NOT IN ('done','failed');

-- 005_create_upload_parts.sql
CREATE TABLE upload_parts (
    upload_id       UUID NOT NULL REFERENCES uploads(upload_id) ON DELETE CASCADE,
    part_number     SMALLINT NOT NULL,
    part_size       INTEGER NOT NULL,
    part_md5        TEXT NOT NULL,          -- base64 MD5 of encrypted part
    etag            TEXT,                   -- S3 ETag, populated on success
    status          TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending','uploaded')),
    PRIMARY KEY (upload_id, part_number)
);

-- 006_create_files.sql
CREATE TABLE files (
    file_id                 BIGSERIAL PRIMARY KEY,
    user_id                 TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    upload_id               UUID REFERENCES uploads(upload_id),
    collection_id           TEXT NOT NULL,
    object_key              TEXT NOT NULL,
    file_size               BIGINT NOT NULL,
    mime_type               TEXT NOT NULL,
    cipher                  TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    encrypted_key           TEXT NOT NULL,
    key_decryption_nonce    TEXT NOT NULL,
    file_decryption_header  TEXT NOT NULL,
    thumb_decryption_header TEXT,
    encrypted_metadata      TEXT NOT NULL,
    encrypted_thumbnail     TEXT,
    thumbnail_size          INTEGER,
    content_hash            TEXT NOT NULL,
    updation_time           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at             TIMESTAMPTZ
);

CREATE INDEX idx_files_user_id ON files(user_id);
CREATE INDEX idx_files_collection ON files(user_id, collection_id);
CREATE INDEX idx_files_content_hash ON files(user_id, content_hash);
CREATE INDEX idx_files_object_key ON files(object_key);
CREATE INDEX idx_files_updation ON files(user_id, updation_time);
CREATE INDEX idx_files_active ON files(user_id, collection_id)
    WHERE archived_at IS NULL;

-- 007_create_shares.sql
CREATE TABLE shares (
    file_id         BIGINT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    shared_with     TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    collection_id   TEXT NOT NULL,
    -- CollectionKey encrypted with recipient's X25519 PublicKey (Req 6.1, 6.4)
    encrypted_collection_key TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ,
    PRIMARY KEY (file_id, shared_with)
);

CREATE INDEX idx_shares_shared_with ON shares(shared_with);
CREATE INDEX idx_shares_expires ON shares(expires_at) WHERE expires_at IS NOT NULL;
```

### 5.3 Upload State Machine

Fixes NITPICKS.md #45: `STALLED → ENCRYPTING` is removed. A stalled upload resumes to `UPLOADING` only, because encryption was already completed before the stall (Req 13.3).

| From | To | Trigger | Who | Notes |
|------|----|---------|-----|-------|
| PENDING | ENCRYPTING | PATCH `{status:"encrypting"}` | Client | Req 13.1 |
| ENCRYPTING | UPLOADING | PATCH `{status:"uploading"}` | Client | Req 13.1 |
| UPLOADING | UPLOADING | PATCH `{parts_bitmask}` | Client | Heartbeat/progress (Req 13.4, 13.6) |
| UPLOADING | S3_COMPLETED | PATCH `{status:"s3_completed"}` | Client | Req 13.1 |
| S3_COMPLETED | REGISTERING | POST `.../register` | Client | Req 13.1 |
| REGISTERING | DONE | DB insert + SSE broadcast | Server-internal | Req 13.1, 15.3 |
| UPLOADING | STALLED | Heartbeat timeout (Stall_Detector) | Server worker | Req 16.1 |
| ENCRYPTING | STALLED | Heartbeat timeout (Stall_Detector) | Server worker | Req 13.1 |
| **STALLED** | **UPLOADING** | PATCH `{status:"resuming"}` | Client | **Req 13.3 — NOT ENCRYPTING** |
| STALLED | FAILED | DELETE (cancel) | Client | Req 13.1 |
| STALLED | FAILED | GC: `expires_at` reached | Server worker | Req 13.1, 18.1 |
| UPLOADING | FAILED | S3 error / AbortMultipartUpload | Client | Req 13.1 |
| any active | FAILED | DELETE (cancel) | Client | Req 27 |

**Invalid transitions** return HTTP 409 Conflict (Req 13.7). The server validates every PATCH against this table (Req 13.2).

### 5.4 API Routes

All endpoints require `Authorization: Bearer <session_token>` except `/api/auth/*` and `/api/events` (which uses `Authorization: Bearer <sse_token>`).

| Method | Path | Auth | Request Body | Response | Error Codes |
|--------|------|------|-------------|----------|-------------|
| POST | `/api/auth/params` | None | `{email}` | `{kek_salt, mem_limit, ops_limit}` | — |
| POST | `/api/auth/login` | None | `{email, verify_key_hash}` | `{session_token, key_attributes}` | 401 |
| DELETE | `/api/auth/logout` | Session | — | 200 | 401 |
| POST | `/api/auth/register` | None | `{email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce}` | `{user_id}` | 409 |
| POST | `/api/devices` | Session | `{name, platform, push_token?}` | `{device_id, sse_token}` | 409 |
| PATCH | `/api/devices/me` | Session | `{name?, push_token?, stall_timeout_seconds?}` | 200 | 401 |
| DELETE | `/api/devices/me` | Session | — | 200 | 401 |
| POST | `/api/uploads` | Session | `{file_hash, file_size, mime_type, collection_id, metadata}` | `{upload_id, status, device_name}` | 409 |
| PATCH | `/api/uploads/{id}` | Session | `{status?, parts_bitmask?}` | UploadState | 400, 404, 409 |
| POST | `/api/uploads/{id}/presign` | Session | `{part_size, part_md5s[]}` | `{object_key, part_urls[], complete_url, urls_expire_at}` | 400, 404 |
| POST | `/api/uploads/{id}/presign-refresh` | Session | `{}` | `{part_urls[], complete_url, urls_expire_at}` | 400, 404 |
| POST | `/api/uploads/{id}/register` | Session | `{encrypted_key, key_decryption_nonce, file_decryption_header, thumb_decryption_header?, encrypted_metadata, encrypted_thumbnail?, collection_id, file_size, thumbnail_size?, mime_type}` | `{file_id}` | 400, 404 |
| DELETE | `/api/uploads/{id}` | Session | — | 200 | 404 |
| GET | `/api/uploads` | Session | `?status=active\|stalled\|all` | `[UploadSummary]` | 401 |
| GET | `/api/uploads/{id}` | Session | — | UploadState + parts | 404 |
| GET | `/api/files/{id}/download` | Session | — | 302 redirect or stream | 403, 404 |
| GET | `/api/files/{id}/thumbnail` | Session | — | 302 redirect or stream | 403, 404 |
| GET | `/api/sync/collections` | Session | `?since=<cursor>` | SyncCollectionResponse | 401 |
| GET | `/api/sync/files` | Session | `?collection_id=<id>&since=<cursor>&limit=1000` | SyncFilesResponse | 401 |
| GET | `/api/sync/trash` | Session | `?since=<cursor>` | SyncTrashResponse | 401 |
| GET | `/api/events` | SseToken | — | `text/event-stream` | 401 |

### 5.5 SSE Hub Design

Implements Req 19.6, 19.7.

```rust
// crates/zoo/src/sse/hub.rs

use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::broadcast;

/// Per-user broadcast channel map.
/// Buffer capacity: 256 events per user (Req 19.6).
/// Slow consumers are dropped (lagged) rather than blocking the broadcast.
pub struct SseHub {
    channels: RwLock<HashMap<String, broadcast::Sender<SseEvent>>>,
}

impl SseHub {
    pub fn new() -> Self {
        Self { channels: RwLock::new(HashMap::new()) }
    }

    /// Subscribe a new device to the user's event stream.
    pub fn subscribe(&self, user_id: &str) -> broadcast::Receiver<SseEvent> {
        let mut channels = self.channels.write().unwrap();
        let sender = channels
            .entry(user_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0);
        sender.subscribe()
    }

    /// Broadcast an event to all connected devices for a user.
    /// Ignores RecvError (no active receivers).
    pub fn broadcast(&self, user_id: &str, event: SseEvent) {
        if let Some(sender) = self.channels.read().unwrap().get(user_id) {
            let _ = sender.send(event);
        }
    }

    /// Remove the channel when the last subscriber disconnects.
    pub fn cleanup_if_empty(&self, user_id: &str) {
        let mut channels = self.channels.write().unwrap();
        if let Some(sender) = channels.get(user_id) {
            if sender.receiver_count() == 0 {
                channels.remove(user_id);
            }
        }
    }
}
```

**Postgres LISTEN/NOTIFY fan-out for multi-replica deployments (Req 19.7):**

```rust
// Each Zoo replica runs a background task:
// 1. LISTEN events;
// 2. On NOTIFY: parse JSON payload → SseEvent → hub.broadcast(user_id, event)
// 3. On any state-changing operation: NOTIFY events, '<json>';

// Publishing (in any handler that changes upload state):
sqlx::query("SELECT pg_notify('events', $1)")
    .bind(serde_json::to_string(&NotifyPayload { user_id, event })?)
    .execute(&pool)
    .await?;

// Listening (background task per replica):
let mut listener = PgListener::connect_with(&pool).await?;
listener.listen("events").await?;
loop {
    let notification = listener.recv().await?;
    let payload: NotifyPayload = serde_json::from_str(notification.payload())?;
    hub.broadcast(&payload.user_id, payload.event);
}
```

### 5.6 Stall Detector

Implements Req 16.1–16.5. Uses `SKIP LOCKED` for multi-replica safety (Req 16.4).

```sql
-- Runs every 15 seconds (Req 16.1)
-- Uses per-device stall_timeout_seconds (Req 16.5)
SELECT u.upload_id, u.user_id, u.device_id, u.parts_bitmask, u.part_count,
       d.stall_timeout_seconds, d.push_token
FROM uploads u
JOIN devices d ON d.device_id = u.device_id
WHERE u.status IN ('uploading', 'encrypting')
  AND u.last_heartbeat_at < NOW() - (d.stall_timeout_seconds || ' seconds')::INTERVAL
FOR UPDATE OF u SKIP LOCKED;
```

```
Worker loop (runs every 15 seconds):
  BEGIN TRANSACTION
  rows = execute stall query above
  FOR EACH row:
    UPDATE uploads
      SET status = 'stalled',
          stalled_at = NOW(),
          expires_at = NOW() + INTERVAL '7 days'
      WHERE upload_id = row.upload_id
    hub.broadcast(row.user_id, SseEvent::UploadStalled { ... })
    IF row.push_token IS NOT NULL:
      enqueue_push_notification(row.device_id, row.push_token, ...)  // Req 16.3
  COMMIT
```

### 5.7 GC Worker

Implements Req 18.1–18.7. Uses `SKIP LOCKED` for multi-replica safety (Req 18.5).

```sql
-- Runs every 5 minutes (Req 18.1)
SELECT upload_id, user_id, object_key, upload_id_s3
FROM uploads
WHERE status NOT IN ('done', 'failed')
  AND expires_at < NOW()
FOR UPDATE SKIP LOCKED;
```

```
Worker loop (runs every 5 minutes):
  BEGIN TRANSACTION
  rows = execute GC query above
  FOR EACH row:
    IF row.upload_id_s3 IS NOT NULL:
      s3.abort_multipart_upload(row.object_key, row.upload_id_s3)  // Req 18.2
    ELSE IF row.object_key IS NOT NULL AND no_parts_in_s3(row):
      s3.delete_object(row.object_key)                              // Req 18.3
    UPDATE uploads
      SET status = 'failed',
          error_reason = 'gc_expired'
      WHERE upload_id = row.upload_id                               // Req 18.4
    hub.broadcast(row.user_id, SseEvent::UploadFailed {
      reason: "expired", ...
    })
  COMMIT
```

**Expiry schedule (Req 18.6):**

| Status | Expiry | Set when |
|--------|--------|----------|
| PENDING | `created_at + 1 hour` | On creation |
| ENCRYPTING | `transition_at + 24 hours` | On PENDING→ENCRYPTING |
| UPLOADING | `NOW() + 24 hours` | Reset on every heartbeat PATCH |
| STALLED | `stalled_at + 7 days` | On UPLOADING→STALLED |
| S3_COMPLETED | `transition_at + 1 hour` | On UPLOADING→S3_COMPLETED |
| REGISTERING | `transition_at + 1 hour` | On S3_COMPLETED→REGISTERING |
| DONE | Immediate cleanup | After file record inserted |
| FAILED | `failed_at + 24 hours` | On any →FAILED transition |

### 5.8 Auth Flow

**Registration flow (Req 1.9–1.12):**

```
Client                                    Zoo
  │                                         │
  ├─ POST /api/auth/register ──────────────→│
  │   { email, verify_key_hash,             │
  │     encrypted_master_key, key_nonce,    │
  │     kek_salt, mem_limit, ops_limit,     │
  │     public_key, encrypted_secret_key,  │
  │     secret_key_nonce,                  │
  │     encrypted_recovery_key,            │
  │     recovery_key_nonce }               │
  │                                         │
  │                          IF email exists: 409 Conflict
  │                          ELSE:
  │                            store bcrypt(verify_key_hash)  ← Req 1.10
  │                            store all encrypted keys
  │                            return { user_id }
  │← { user_id } ──────────────────────────│
```

**Login flow (Req 2.1–2.10):**

```
Client                                    Zoo
  │                                         │
  ├─ POST /api/auth/params ────────────────→│
  │   { email }                             │
  │                          Returns SAME params regardless of
  │                          email existence (Req 2.1 — anti-enumeration)
  │← { kek_salt, mem_limit, ops_limit } ───│
  │                                         │
  ├─ Argon2id(password, kek_salt) → KEK    │
  ├─ BLAKE2b-KDF(KEK, "verification") → VK │
  ├─ SHA-256(VK) → verify_key_hash         │
  │                                         │
  ├─ POST /api/auth/login ─────────────────→│
  │   { email, verify_key_hash }            │
  │                          1. Lookup user by email
  │                          2. constant_time_eq(                ← Req 2.4, NITPICKS #26
  │                               bcrypt_verify(verify_key_hash,
  │                               stored_hash))
  │                          3. If match:
  │                             - generate 32-byte random SessionToken
  │                             - store SHA-256(SessionToken) → user_id
  │                             - return token + key_attributes
  │                          4. If no match: 401 (no email hint)  ← Req 2.7
  │← { session_token, key_attributes } ────│
  │                                         │
  ├─ XSalsa20-Poly1305 decrypt(             │
  │   encrypted_master_key, key_nonce, KEK) │
  │   → MasterKey                           │
  ├─ Store MasterKey in secret memory       │  ← Req 2.11
```

**Constant-time comparison note (Req 2.4, NITPICKS #26):** The `bcrypt_verify` function must use a constant-time comparison internally. Additionally, for non-existent emails, Zoo must run a dummy `bcrypt_verify` with fixed parameters to prevent timing-based user enumeration. The dummy parameters are stored as a server-side constant and never change.


---

### Sync Engine (`crates/sync`)

Implements Req 7, 8.

### 6.1 Module Structure

```
crates/sync/
├── Cargo.toml            # deps: types, crypto, metadata, thumbnail, local-db, common
└── src/
    ├── lib.rs            # Public API: sync_all(), SyncEngine struct
    ├── pull.rs           # Top-level sync_all() orchestration loop
    ├── diff.rs           # Fetch and process a single diff page from Zoo
    ├── decrypt.rs        # Batch decrypt EncryptedFileRecord → FileRecord
    ├── thumbnails.rs     # Trigger thumbnail download for newly synced files
    └── cursor.rs         # Read/write sync cursors from local-db sync_state table
```

### 6.2 Sync Flow

Implements Req 7.1–7.7, 8.1–8.8.

```
async fn sync_all(engine: &SyncEngine) -> Result<(), SyncError>:

  // Step 1: Sync collections (Req 7.1)
  collection_cursor = cursor::read("collections_since").unwrap_or(0)
  loop:
    resp = zoo_client.get_sync_collections(since=collection_cursor).await?
    for collection in resp.collections:
      // Decrypt collection name and key with MasterKey (Req 7.4)
      name = crypto::secretbox_decrypt(
               &collection.name_decryption_nonce,
               &collection.encrypted_name,
               &master_key)?
      collection_key = crypto::secretbox_decrypt(
               &collection.key_decryption_nonce,
               &collection.encrypted_key,
               &master_key)?
      local_db.upsert_collection(Collection {
        id: collection.id,
        name: String::from_utf8(name)?,
        encrypted_key: collection.encrypted_key,
        key_nonce: collection.key_decryption_nonce,
        updation_time: collection.updation_time,
        ...
      }).await?
    // Persist cursor after each page (Req 7.5)
    cursor::write("collections_since", resp.latest_updated_at).await?
    collection_cursor = resp.latest_updated_at
    if !resp.has_more: break  // Req 7.6

  // Step 2: Sync files per collection (Req 8.1)
  collections = local_db.list_collections().await?
  for collection in collections:
    collection_key = local_db.get_collection_key(collection.id).await?
    file_cursor = cursor::read(format!("collection:{}_since", collection.id))
                    .unwrap_or(0)
    loop:
      resp = zoo_client.get_sync_files(
               collection_id=collection.id,
               since=file_cursor,
               limit=1000).await?
      // Decrypt batch (Req 8.3)
      records = decrypt::batch_decrypt_files(&resp.updated_files, &collection_key)?
      local_db.upsert_files(records).await?
      // Handle deletions (Req 8.4)
      local_db.archive_files(resp.deleted_file_ids).await?
      // Persist cursor (Req 8.5)
      cursor::write(
        format!("collection:{}_since", collection.id),
        resp.latest_updated_at).await?
      file_cursor = resp.latest_updated_at
      if !resp.has_more: break  // Req 8.6

  // Step 3: Sync trash (Req 8.7)
  trash_cursor = cursor::read("trash_since").unwrap_or(0)
  loop:
    resp = zoo_client.get_sync_trash(since=trash_cursor).await?
    local_db.archive_files(resp.deleted_files.map(|f| f.file_id)).await?
    cursor::write("trash_since", resp.latest_updated_at).await?
    trash_cursor = resp.latest_updated_at
    if !resp.has_more: break

  // Step 4: Trigger thumbnail downloads for new files
  thumbnails::queue_new_files(&local_db).await?
```

**First Sync vs Incremental (SPEC §2.6):**

| Scenario | Cursor | Behavior |
|---|---|---|
| First sync (new device) | `since=0` | Full download of all metadata + thumbnails |
| Normal sync | `since=last_sync` | Only files with `updation_time > cursor` |
| Re-login / clear data | Reset to `since=0` | Metadata fetched from scratch (thumbnails can reuse disk cache by file ID) |
| Offline | No sync | All operations against local DB. Queued changes saved for next sync. |

**Offline behavior (Req 8.8, SPEC §2.7):** `sync_all()` is only called when the network is available. All read operations (browse, search, grid view) go directly to `local_db` without calling `sync_all()`. The caller (client-lib) checks connectivity before invoking sync. When offline:
- All metadata is local — full search, browse, and grid view work offline
- Cached thumbnails (L1 memory + L2 disk) render the grid
- Opening a full image requires network (unless previously cached for offline)
- Uploads are queued locally and sent when online
- Queued changes (new collections, archived files) are saved to local DB and applied on next sync

### 6.3 Version-Consistent Pagination

Implements Req 7.3, SPEC §2.3. Prevents splitting a group of records with the same `updation_time` across two pages.

```
Server-side algorithm (in zoo/src/api/sync.rs):

fn paginate_files(
    collection_id: &str,
    since: i64,
    limit: usize,
) -> (Vec<EncryptedFileRecord>, bool, i64):

  // Fetch N+1 rows to detect if there are more
  rows = db.query(
    "SELECT * FROM files
     WHERE collection_id = $1
       AND updation_time > $2
       AND archived_at IS NULL
     ORDER BY updation_time ASC, file_id ASC
     LIMIT $3",
    collection_id, since, limit + 1
  )

  if rows.len() <= limit:
    // All rows fit — no more pages
    latest = rows.last().map(|r| r.updation_time).unwrap_or(since)
    return (rows, false, latest)

  // We have limit+1 rows — there are more pages
  // Find the boundary: discard the last group sharing the same updation_time
  // to avoid splitting a version group (Req 7.3)
  rows = rows[..limit]  // take first N
  last_time = rows.last().unwrap().updation_time

  // Walk back to find where this time group starts
  cutoff = rows.partition_point(|r| r.updation_time < last_time)
  if cutoff == 0:
    // All rows have the same updation_time — return them all anyway
    // (edge case: entire page is one version group)
    latest = last_time
    return (rows, true, latest)

  rows = rows[..cutoff]
  latest = rows.last().unwrap().updation_time
  return (rows, true, latest)
```

**Note on `updation_time` precision (NITPICKS #36):** `updation_time` is stored as `TIMESTAMPTZ` in PostgreSQL (microsecond precision). The secondary sort key `file_id` (a BIGSERIAL) ensures deterministic ordering within the same microsecond, preventing the edge case where N+1 rows all share the same timestamp.

### 6.4 Decryption Pipeline

Implements Req 8.3, 4.6.

```
fn batch_decrypt_files(
    records: &[EncryptedFileRecord],
    collection_key: &Key256,
) -> Result<Vec<FileRecord>, SyncError>:

  records.iter().map(|record| -> Result<FileRecord, SyncError>:
    // Step 1: Decrypt FileKey with CollectionKey (Req 4.6, 8.3)
    file_key_bytes = crypto::secretbox_decrypt(
      &Nonce24::from_base64(&record.key_decryption_nonce)?,
      &base64::decode(&record.encrypted_key)?,
      collection_key,
    ).map_err(|e| SyncError::DecryptionFailed {
      file_id: record.id, source: e
    })?

    file_key = Key256::from_bytes(file_key_bytes)?

    // Step 2: Decrypt metadata with FileKey (Req 8.3)
    metadata_bytes = crypto::stream_decrypt(
      &Header24::from_base64(&record.file_decryption_header)?,
      &base64::decode(&record.encrypted_metadata)?,
      &file_key,
    ).map_err(|e| SyncError::DecryptionFailed {
      file_id: record.id, source: e
    })?

    metadata: FileMetadata = serde_json::from_slice(&metadata_bytes)?

    Ok(FileRecord {
      id: record.id,
      collection_id: record.collection_id.clone(),
      cipher: record.cipher.clone(),
      title: metadata.title,
      description: metadata.description,
      latitude: metadata.latitude,
      longitude: metadata.longitude,
      taken_at: metadata.taken_at,
      file_size: record.file_size,
      mime_type: record.mime_type.clone(),
      content_hash: record.content_hash.clone(),
      encrypted_key: record.encrypted_key.clone(),
      key_nonce: record.key_decryption_nonce.clone(),
      file_decryption_header: record.file_decryption_header.clone(),
      thumb_decryption_header: record.thumb_decryption_header.clone(),
      object_key: record.object_key.clone(),
      thumbnail_path: None,
      updation_time: record.updation_time,
      created_at: record.created_at,
      archived_at: record.archived_at,
    })
  ).collect()
```

**Decryption failure handling (Req 7.7):** If decryption fails for a record, `SyncError::DecryptionFailed` is logged and the record is skipped. The sync continues with remaining records. The cursor is still advanced past the failed record to prevent infinite retry loops.


---

### Local DB (`crates/local-db`)

Implements Req 9, 21.

### 7.1 Module Structure

```
crates/local-db/
├── Cargo.toml            # deps: types, common, rusqlite (with sqlcipher feature),
│                         #   rusqlite-migration
└── src/
    ├── lib.rs            # Public API: LocalDb struct, open(), close()
    ├── connection.rs     # Open/close SQLite DB with SQLCipher key; run migrations (Req 9.5)
    ├── migrations/
    │   ├── mod.rs        # Embedded migration runner (rusqlite-migration)
    │   ├── 001_initial.sql
    │   └── 002_fts5.sql  # FTS5 virtual table (Req 21.1, NITPICKS #28/#39)
    ├── collections.rs    # CRUD: collections table
    ├── files.rs          # CRUD: files table; upsert, archive, list
    ├── sync_state.rs     # Read/write sync cursors (key-value)
    └── search.rs         # FTS5 text search, date range, geographic bounding box
```

### 7.2 SQLite Schema

Implements Req 9.1–9.4. Fixes NITPICKS.md #28/#39: uses FTS5 virtual table for text search instead of `LIKE '%query%'` (Req 21.1).

```sql
-- 001_initial.sql

-- Collections (decrypted locally after sync) (Req 9.1)
CREATE TABLE collections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,          -- decrypted collection name
    encrypted_key   TEXT NOT NULL,          -- kept for re-encryption / sharing
    key_nonce       TEXT NOT NULL,
    updation_time   INTEGER NOT NULL,       -- Unix ms
    created_at      INTEGER NOT NULL,
    archived_at     INTEGER                 -- NULL = active
);

-- Files (decrypted locally after sync) (Req 9.2)
CREATE TABLE files (
    id                      INTEGER PRIMARY KEY,
    collection_id           TEXT NOT NULL REFERENCES collections(id),
    cipher                  TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    title                   TEXT,
    description             TEXT,
    latitude                REAL,
    longitude               REAL,
    taken_at                INTEGER,        -- Unix ms
    file_size               INTEGER NOT NULL,
    mime_type               TEXT NOT NULL,
    content_hash            TEXT NOT NULL,
    encrypted_key           TEXT NOT NULL,
    key_nonce               TEXT NOT NULL,
    file_decryption_header  TEXT NOT NULL,
    thumb_decryption_header TEXT,
    object_key              TEXT NOT NULL,
    thumbnail_path          TEXT,           -- local cache path (Req 11.2)
    updation_time           INTEGER NOT NULL,
    created_at              INTEGER NOT NULL,
    archived_at             INTEGER         -- NULL = active
);

-- Required indexes (Req 9.4)
CREATE INDEX idx_files_collection ON files(collection_id);
CREATE INDEX idx_files_taken_at ON files(taken_at);
CREATE INDEX idx_files_archived ON files(archived_at) WHERE archived_at IS NULL;
-- Additional indexes for search performance
CREATE INDEX idx_files_latitude ON files(latitude) WHERE latitude IS NOT NULL;
CREATE INDEX idx_files_longitude ON files(longitude) WHERE longitude IS NOT NULL;

-- Sync cursors (Req 9.3)
-- Keys: "collections_since", "collection:{id}_since", "trash_since"
CREATE TABLE sync_state (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

-- 002_fts5.sql
-- FTS5 virtual table for full-text search (Req 21.1, fixes NITPICKS #28/#39)
-- content= makes it a content table backed by the files table
CREATE VIRTUAL TABLE files_fts USING fts5(
    title,
    description,
    content='files',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 1'
);

-- Triggers to keep FTS5 index in sync with files table
CREATE TRIGGER files_fts_insert AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER files_fts_update AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER files_fts_delete AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
END;
```

### 7.3 Encryption at Rest

Implements Req 9.6, 9.7, 22.6.

The SQLite database file is encrypted using **SQLCipher** (the `rusqlite` crate with the `sqlcipher` feature). SQLCipher applies AES-256-CBC to every 4 KB page of the database file.

The encryption key is a 32-byte random key stored in the platform credential store:

| Platform | Credential Store | API |
|----------|-----------------|-----|
| macOS | Keychain Services | `security` crate or `keyring` crate |
| Windows | DPAPI (Data Protection API) | `winapi` / `keyring` crate |
| Linux | Secret Service (libsecret) | `keyring` crate |

```rust
// crates/local-db/src/connection.rs

pub fn open(db_path: &Path) -> Result<LocalDb, LocalDbError> {
    // 1. Retrieve or generate the DB encryption key from platform keychain
    let key = platform_keychain::get_or_create_db_key()
        .map_err(|_| LocalDbError::KeychainUnavailable)?;  // Req 9.7

    // 2. Open SQLite with SQLCipher key
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "key", &hex::encode(&key))?;

    // 3. Verify the key works (SQLCipher returns error on wrong key)
    conn.pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
        .map_err(|_| LocalDbError::InvalidKey)?;

    // 4. Run pending migrations (Req 9.5)
    let migrations = Migrations::new(MIGRATIONS.to_vec());
    migrations.to_latest(&mut conn)?;

    Ok(LocalDb { conn })
}
```

**Key binding:** The key is bound to the device, not the user account. If the user logs out and logs back in, the same key is used (the DB is not re-encrypted). If the device is wiped, the key is lost and the DB must be re-synced from the server.

### 7.4 Search Queries

Implements Req 21.2–21.4.

```sql
-- FTS5 full-text search (Req 21.1, 21.2)
-- Fixes NITPICKS #28/#39: uses FTS5 instead of LIKE '%query%'
SELECT f.*
FROM files f
JOIN files_fts fts ON fts.rowid = f.id
WHERE files_fts MATCH ?          -- FTS5 query syntax (e.g., "beach sunset")
  AND f.archived_at IS NULL
ORDER BY f.taken_at DESC
LIMIT 50;

-- Date range search (Req 21.3)
SELECT *
FROM files
WHERE taken_at BETWEEN ? AND ?
  AND archived_at IS NULL
ORDER BY taken_at DESC
LIMIT 50;

-- Geographic bounding box search (Req 21.4)
SELECT *
FROM files
WHERE latitude  BETWEEN ? AND ?
  AND longitude BETWEEN ? AND ?
  AND archived_at IS NULL
ORDER BY taken_at DESC
LIMIT 50;
```

**FTS5 index rebuild (Req 21.6):** After a bulk sync that inserts many rows, the triggers above keep the FTS5 index in sync automatically. If the index becomes inconsistent (e.g., after a crash mid-sync), the `search.rs` module calls `INSERT INTO files_fts(files_fts) VALUES('rebuild')` before returning results.


---

### Thumbnail Pipeline (`crates/thumbnail`)

Implements Req 10, 11.

### 8.1 Module Structure

```
crates/thumbnail/
├── Cargo.toml            # deps: types, crypto, image, metadata
└── src/
    ├── lib.rs            # Public API: ThumbnailPipeline struct
    ├── generate.rs       # Image/video → resized JPEG ≤720px ≤100KB (Req 10.1–10.5)
    ├── encrypt.rs        # Raw thumbnail bytes → XChaCha20-Poly1305 (Req 10.6)
    ├── decrypt.rs        # Encrypted thumbnail → raw bytes (Req 10.7)
    ├── cache/
    │   ├── mod.rs        # ThumbnailCache: two-level cache coordinator (Req 11.1)
    │   ├── memory.rs     # Level 1: LRU in-memory cache, 500 items (Req 11.1)
    │   └── disk.rs       # Level 2: disk LRU cache, 2 GB max (Req 11.1, 11.7)
    ├── download.rs       # Fetch from Zoo, decrypt, populate both cache levels (Req 11.4)
    └── inflight.rs       # DashMap<FileId, Arc<Notify>> for request deduplication (Req 11.5)
```

### 8.2 Generation Pipeline

Implements Req 10.1–10.8, 26.2.

```
fn generate_thumbnail(
    source: &[u8],
    mime_type: &str,
    file_key: &Key256,
) -> Result<(Header24, Vec<u8>), ThumbnailError>:

  // Step 1: Decode image
  image = image::decode(source, mime_type)?
  // Returns ThumbnailError::UnsupportedFormat on failure (Req 10.8)

  // Step 2: Apply EXIF orientation correction (Req 10.5, 26.2)
  orientation = image::exif::extract_orientation(source)
  image = image::orientation::apply(image, orientation)

  // Step 3: Resize to max 720px, preserving aspect ratio (Req 10.1)
  image = image::resize::max_dimension(image, 720)

  // Step 4: Encode as JPEG at quality 85 (Req 10.2)
  bytes = image::encode::jpeg(image, quality=85)

  // Step 5: Iterative quality reduction if > 100 KB (Req 10.4)
  quality = 85
  while bytes.len() > 100 * 1024 && quality > 10:
    quality -= 10
    bytes = image::encode::jpeg(image, quality)

  if bytes.len() > 100 * 1024:
    // Even at quality 10, still too large — use quality 10 anyway
    // (extremely rare for a 720px image)
    bytes = image::encode::jpeg(image, quality=10)

  // Step 6: Encrypt with FileKey (Req 10.6)
  (header, ciphertext) = crypto::stream_encrypt(&bytes, file_key)

  Ok((header, ciphertext))
```

**Video thumbnails (Req 10.3):** Video frame extraction at the 1-second mark requires a video decoding library (e.g., `ffmpeg` via `ffmpeg-next` crate). The `generate.rs` module dispatches on `mime_type`: `image/*` uses `image-rs`, `video/*` uses `ffmpeg`. If `ffmpeg` is unavailable or the video is corrupt, `ThumbnailError::UnsupportedFormat` is returned and the upload proceeds without a thumbnail (Req 10.8).

### 8.3 Cache Design

Implements Req 11.1–11.8.

```rust
// crates/thumbnail/src/cache/mod.rs

pub struct ThumbnailCache {
    memory: MemoryCache,    // Level 1: LRU, 500 items (Req 11.1)
    disk: DiskCache,        // Level 2: LRU, 2 GB max (Req 11.1)
    inflight: InflightMap,  // DashMap<FileId, Arc<Notify>> (Req 11.5)
}

impl ThumbnailCache {
    /// Get a thumbnail, populating cache levels as needed.
    pub async fn get(
        &self,
        file_id: FileId,
        file_key: &Key256,
        thumb_header: &Header24,
        zoo_client: &ZooClient,
    ) -> Result<Vec<u8>, ThumbnailError> {
        // Level 1: memory cache (Req 11.2)
        if let Some(bytes) = self.memory.get(&file_id) {
            return Ok(bytes);
        }

        // Level 2: disk cache (Req 11.3)
        if let Some(bytes) = self.disk.get(&file_id).await? {
            self.memory.insert(file_id, bytes.clone());
            return Ok(bytes);
        }

        // Deduplication: only one in-flight request per file_id (Req 11.5)
        let notify = self.inflight.get_or_insert(file_id);
        if notify.is_existing() {
            // Another task is already downloading — wait for it
            notify.notified().await;
            // Now check cache again
            if let Some(bytes) = self.memory.get(&file_id) {
                return Ok(bytes);
            }
            return self.disk.get(&file_id).await?.ok_or(ThumbnailError::NotFound);
        }

        // Cache miss: download, decrypt, populate both levels (Req 11.4)
        let encrypted = zoo_client.get_thumbnail(file_id).await?;
        let bytes = crypto::stream_decrypt(thumb_header, &encrypted, file_key)?;

        self.disk.insert(file_id, &bytes).await?;  // Req 11.4
        self.memory.insert(file_id, bytes.clone()); // Req 11.4

        // Notify waiting tasks
        self.inflight.remove_and_notify(file_id);

        Ok(bytes)
    }

    /// Evict a file from both cache levels (Req 11.6).
    pub async fn evict(&self, file_id: FileId) {
        self.memory.remove(&file_id);
        self.disk.remove(&file_id).await;
    }
}
```

**Memory cache (Req 11.1):** `MemoryCache` wraps the `lru` crate with capacity 500. Each entry is the raw decrypted JPEG bytes (~100 KB average → ~50 MB total).

**Disk cache (Req 11.1, 11.7):** `DiskCache` stores files at `{app_cache_dir}/thumbnails/{file_id}`. It maintains a metadata index (file_id → (path, size, last_accessed)) in a small SQLite file. When total size exceeds 2 GB, it evicts the least-recently-used entries until below 2 GB.

**In-flight deduplication (Req 11.5):** `InflightMap` is a `DashMap<FileId, Arc<tokio::sync::Notify>>`. When a download starts, it inserts a `Notify`. Concurrent requests for the same `file_id` wait on the `Notify`. When the download completes, `notify_waiters()` is called and the entry is removed.

**Cache miss on evicted disk entry (Req 11.8):** If `disk.get()` returns `None` (entry was evicted by the OS or user), the code falls through to the download path transparently.

**Cache Invalidation (SPEC §3.4):**

| Event | Action |
|---|---|
| File deleted/archived | Remove thumbnail from disk + memory cache via `ThumbnailCache::evict(file_id)` |
| Thumbnail re-uploaded | Invalidate cache entry, re-download on next view |
| App cache cleared | All thumbnails re-downloaded on demand |
| Disk space low | OS may evict cache directory; handled as cache miss (falls through to download path) |


---

### Zoo Client (`crates/zoo-client`)

Implements Req 14.5–14.6, 17.2–17.7, 23.3–23.4, 27.5.

### 9.1 Module Structure

```
crates/zoo-client/
├── Cargo.toml            # deps: types, reqwest (no default features)
│                         # NO crypto, image, metadata, thumbnail (Req 25.6)
└── src/
    ├── lib.rs            # Public API: ZooClient struct
    ├── orchestrator.rs   # Full upload lifecycle state machine (Req 14, 17)
    ├── upload.rs         # Multipart S3 upload: PUT parts, complete, abort
    ├── download.rs       # Download file/thumbnail from Zoo redirect/proxy
    ├── sse.rs            # SSE event stream client; reconnect logic
    └── types.rs          # Re-exported protocol types from crates/types
```

### 9.2 Upload Orchestrator

Implements Req 12.4, 13, 14, 15, 17. Fixes NITPICKS #53: handles 403 from S3 by calling `presign-refresh`.

```
async fn upload_file(
    client: &ZooClient,
    source_bytes: &[u8],          // already-encrypted file bytes
    metadata: UploadMetadata,
    file_key: &Key256,            // for thumbnail encryption (passed in, not stored)
) -> Result<FileId, ZooError>:

  // Step 1: Create upload record (Req 12.4)
  upload = client.post_uploads(UploadRequest {
    file_hash: sha256(plaintext_bytes),
    file_size: source_bytes.len(),
    mime_type: metadata.mime_type,
    collection_id: metadata.collection_id,
    metadata: metadata.encrypted_metadata,
  }).await?
  // 409 = duplicate upload already exists (Req 12.5)

  // Step 2: Transition to ENCRYPTING (Req 13.1)
  client.patch_upload(upload.id, PatchUpload { status: "encrypting", .. }).await?

  // Step 3: Compute part MD5s
  parts = split_into_parts(source_bytes, DEFAULT_PART_SIZE)
  part_md5s = parts.iter().map(|p| base64(md5(p))).collect()

  // Step 4: Presign (Req 14.1)
  presign = client.post_presign(upload.id, PresignRequest {
    part_size: DEFAULT_PART_SIZE,
    part_md5s,
  }).await?

  // Step 5: Transition to UPLOADING (Req 13.1)
  client.patch_upload(upload.id, PatchUpload {
    status: "uploading",
    parts_bitmask: empty_bitmask(presign.part_count),
  }).await?

  // Step 6: Upload parts with heartbeat (Req 13.6, 14.5)
  bitmask = BitVec::new(presign.part_count)
  etags = Vec::new()
  last_heartbeat = Instant::now()

  for (i, (part, url)) in parts.iter().zip(presign.part_urls.iter()).enumerate():
    // Retry loop for 403 (expired presigned URL) (Req 14.5, NITPICKS #53)
    etag = loop:
      result = s3_put(url, part).await
      match result:
        Ok(etag) → break etag
        Err(S3Error::Forbidden) | Err(S3Error::Expired):
          // Refresh presigned URLs (Req 14.4)
          refresh = client.post_presign_refresh(upload.id).await?
          url = refresh.part_urls[i]
          continue
        Err(other) → return Err(ZooError::S3Error(other))

    bitmask.set(i)
    etags.push(etag)

    // Heartbeat: send PATCH every 30s or every 5 parts (Req 13.6)
    if i % 5 == 0 || last_heartbeat.elapsed() > Duration::from_secs(25):
      client.patch_upload(upload.id, PatchUpload {
        parts_bitmask: bitmask.to_base64(),
        ..
      }).await?
      last_heartbeat = Instant::now()

  // Step 7: Complete multipart upload on S3 (Req 14.6)
  s3_complete(presign.complete_url, &etags).await?

  // Step 8: Transition to S3_COMPLETED (Req 14.6)
  client.patch_upload(upload.id, PatchUpload {
    status: "s3_completed", ..
  }).await?

  // Step 9: Register file record (Req 15.1)
  register_resp = client.post_register(upload.id, RegisterRequest {
    encrypted_key: metadata.encrypted_key,
    key_decryption_nonce: metadata.key_nonce,
    file_decryption_header: metadata.file_header,
    thumb_decryption_header: metadata.thumb_header,
    encrypted_metadata: metadata.encrypted_metadata,
    encrypted_thumbnail: metadata.encrypted_thumbnail,
    collection_id: metadata.collection_id,
    file_size: source_bytes.len(),
    thumbnail_size: metadata.thumbnail_size,
    mime_type: metadata.mime_type,
  }).await?
  // Idempotent: same upload_id always returns same file_id (Req 15.5)

  Ok(register_resp.file_id)
```

### 9.3 Resume Protocol

Implements Req 17.1–17.7. Fixes NITPICKS #45: resumes to UPLOADING, not ENCRYPTING.

```
async fn resume_upload(
    client: &ZooClient,
    upload_id: Uuid,
    source_bytes: &[u8],  // re-encrypted bytes from originating device (Req 17.6)
) -> Result<FileId, ZooError>:

  // Step 1: Transition STALLED → UPLOADING (Req 17.2, 13.3)
  client.patch_upload(upload_id, PatchUpload { status: "resuming", .. }).await?

  // Step 2: Get current upload state
  state = client.get_upload(upload_id).await?

  // Step 3: S3 ListParts reconciliation (Req 17.4)
  s3_parts = s3_list_parts(state.object_key, state.upload_id_s3).await
    .map_err(|e| match e {
      S3Error::NoSuchUpload => ZooError::UploadAborted,  // Req 17.5
      other => ZooError::S3Error(other),
    })?

  zoo_bitmask = BitVec::from_base64(&state.parts_bitmask)

  // Reconcile Zoo bitmask vs S3 ListParts (Req 17.4)
  // See reconciliation table below
  parts_to_upload = reconcile(zoo_bitmask, s3_parts, &state)

  // Step 4: Refresh presigned URLs for pending parts
  refresh = client.post_presign_refresh(upload_id).await?

  // Step 5: Upload missing parts (same heartbeat logic as initial upload)
  // ... (same loop as upload_file above, but only for parts_to_upload)

  // Step 6: Complete + register (same as upload_file steps 7–9)
```

**S3 ListParts reconciliation table (Req 17.4):**

| Zoo bitmask | S3 ListParts | Action |
|-------------|-------------|--------|
| uploaded (1) | part exists, ETag matches DB | Skip — already done |
| uploaded (1) | part missing from S3 | Mark pending in Zoo, re-upload |
| uploaded (1) | part exists, ETag differs | Mark pending in Zoo, re-upload |
| pending (0) | part missing from S3 | Re-upload |
| pending (0) | part exists in S3 | Unexpected — mark uploaded in Zoo, skip |

If S3 returns `NoSuchUpload` (multipart was aborted by GC or manually), the client marks the upload as FAILED and starts a fresh upload (Req 17.5).

### 9.4 Presigned URL Expiry Handling

Implements Req 14.5, NITPICKS #53.

When a PUT to S3 returns HTTP 403 (expired signature or clock skew), the `zoo-client` orchestrator:

1. Calls `POST /api/uploads/{upload_id}/presign-refresh` to get new URLs for all pending parts (Req 14.4).
2. Retries the failed part with the new URL.
3. Continues with the refreshed URLs for all subsequent parts.

The refresh is transparent to the caller — the orchestrator handles it internally within the upload loop. The retry is bounded to 3 attempts per part to prevent infinite loops on persistent S3 errors.


---

### Client Lib (`crates/client-lib`)

Implements Req 22.2–22.7.

### 10.1 Module Structure

```
crates/client-lib/
├── Cargo.toml            # deps: types, sync, local-db, thumbnail, zoo-client
│                         # NO axum, sqlx (postgres), aws-sdk-s3 (Req 25.5)
│                         # feature "desktop" enables tauri dependency
└── src/
    ├── lib.rs            # Module registration; AppState initialization
    ├── state.rs          # AppState struct definition
    └── commands/
        ├── mod.rs        # Register all Tauri commands
        ├── auth.rs       # login, logout, register, get_key_attributes
        ├── collections.rs # list_collections, create_collection, archive_collection
        ├── files.rs      # list_files, get_file, archive_file, download_file
        ├── sync.rs       # trigger_sync, get_sync_status
        ├── upload.rs     # upload_file, cancel_upload, list_pending_uploads
        ├── thumbnails.rs # get_thumbnail, evict_thumbnail
        ├── device.rs     # register_device, get_device_info
        └── search.rs     # search_files, search_by_date, search_by_location
```

### 10.2 Tauri Commands

All commands are registered with `tauri_specta` to generate TypeScript bindings (Req 22.4). The frontend calls these via `invoke()` (Req 22.3).

```rust
// crates/client-lib/src/commands/auth.rs

#[tauri::command]
#[specta::specta]
pub async fn get_auth_params(
    email: String,
    state: tauri::State<'_, AppState>,
) -> Result<KeyParamsResponse, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn login(
    email: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<LoginResult, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn logout(
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn register(
    email: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<RegisterResult, CommandError>;

// crates/client-lib/src/commands/files.rs

#[tauri::command]
#[specta::specta]
pub async fn list_files(
    collection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileRecord>, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn download_file(
    file_id: i64,
    destination: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn archive_file(
    file_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError>;

// crates/client-lib/src/commands/upload.rs

#[tauri::command]
#[specta::specta]
pub async fn upload_file(
    file_path: String,
    collection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<UploadStarted, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn cancel_upload(
    upload_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn list_pending_uploads(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<UploadSummary>, CommandError>;

// crates/client-lib/src/commands/sync.rs

#[tauri::command]
#[specta::specta]
pub async fn trigger_sync(
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, CommandError>;

// crates/client-lib/src/commands/thumbnails.rs

#[tauri::command]
#[specta::specta]
pub async fn get_thumbnail(
    file_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<u8>, CommandError>;

// crates/client-lib/src/commands/search.rs

#[tauri::command]
#[specta::specta]
pub async fn search_files(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileRecord>, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn search_by_date(
    start_ms: i64,
    end_ms: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileRecord>, CommandError>;

#[tauri::command]
#[specta::specta]
pub async fn search_by_location(
    lat_min: f64, lat_max: f64,
    lon_min: f64, lon_max: f64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileRecord>, CommandError>;
```

### 10.3 AppState

```rust
// crates/client-lib/src/state.rs

use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state, held in a Tauri `State<AppState>`.
/// All fields are `Arc<RwLock<...>>` for safe concurrent access from Tauri commands.
pub struct AppState {
    /// SQLite database handle (encrypted with platform keychain key).
    pub db: Arc<LocalDb>,

    /// In-memory master key (secret-protected memory, never written to disk).
    /// None when logged out.
    pub master_key: Arc<RwLock<Option<Key256>>>,

    /// Current session token (opaque, used for all API requests).
    /// None when logged out.
    pub session_token: Arc<RwLock<Option<String>>>,

    /// Device ID and SSE token for the current device.
    pub device_info: Arc<RwLock<Option<DeviceInfo>>>,

    /// Sync engine state: last sync time, in-progress flag.
    pub sync_state: Arc<RwLock<SyncState>>,

    /// Two-level thumbnail cache.
    pub thumbnail_cache: Arc<ThumbnailCache>,

    /// Zoo HTTP client (base URL, session token injected per-request).
    pub zoo_client: Arc<ZooClient>,

    /// App configuration (server URL, cache dir, etc.).
    pub config: Arc<AppConfig>,
}

impl AppState {
    /// Initialize AppState on app startup (Req 22.2).
    pub async fn init(config: AppConfig) -> Result<Self, AppError> {
        let db = LocalDb::open(&config.db_path)?;
        let thumbnail_cache = ThumbnailCache::new(
            500,                          // Level 1: 500 items (Req 11.1)
            config.cache_dir.join("thumbnails"),
            2 * 1024 * 1024 * 1024,      // Level 2: 2 GB (Req 11.1)
        );
        // ... initialize zoo_client, load sync cursors, etc.
        Ok(Self { db: Arc::new(db), master_key: Arc::new(RwLock::new(None)), ... })
    }
}
```


---

### Platform Strategy

### 11.1 Desktop (Tauri)

Implements Req 22.

The Tauri application is a thin shell. `apps/desktop/src-tauri/main.rs` contains no business logic — it imports `client-lib`, registers commands, and starts the Tauri runtime.

```rust
// apps/desktop/src-tauri/src/main.rs

fn main() {
    let config = AppConfig::from_env();
    tauri::Builder::default()
        .manage(AppState::init(config).block_on())
        .invoke_handler(tauri::generate_handler![
            // auth
            client_lib::commands::auth::get_auth_params,
            client_lib::commands::auth::login,
            client_lib::commands::auth::logout,
            client_lib::commands::auth::register,
            // files
            client_lib::commands::files::list_files,
            client_lib::commands::files::download_file,
            client_lib::commands::files::archive_file,
            // upload
            client_lib::commands::upload::upload_file,
            client_lib::commands::upload::cancel_upload,
            client_lib::commands::upload::list_pending_uploads,
            // sync
            client_lib::commands::sync::trigger_sync,
            // thumbnails
            client_lib::commands::thumbnails::get_thumbnail,
            // search
            client_lib::commands::search::search_files,
            client_lib::commands::search::search_by_date,
            client_lib::commands::search::search_by_location,
            // collections
            client_lib::commands::collections::list_collections,
            client_lib::commands::collections::create_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**`tauri_specta` TypeScript binding generation (Req 22.4):**

```rust
// apps/desktop/src-tauri/src/bindings.rs (build-time codegen)

fn main() {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            client_lib::commands::auth::login,
            // ... all commands
        ])
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .unwrap();
}
```

This generates `apps/desktop/src/bindings.ts` with fully typed `invoke()` wrappers. TypeScript compilation fails if Rust types change without regenerating bindings (Req 22.4).

**Platform keychain integration (Req 22.6):**

| Platform | API | Crate |
|----------|-----|-------|
| macOS | Keychain Services | `keyring` crate |
| Windows | DPAPI (`CryptProtectData`) | `keyring` crate |
| Linux | Secret Service (D-Bus) | `keyring` crate |

The `keyring` crate provides a unified API. The DB encryption key is stored under service name `"rey"`, username `"local_db_key"`.

**Crypto execution (Req 22.5):** All encryption/decryption runs as native Rust within the Tauri process. No WASM involved on desktop.

**MasterKey protection (Req 22.7):** The `Key256` type implements `Zeroize` and `ZeroizeOnDrop`. On Linux, `mprotect(PROT_NONE)` is applied to the memory page containing the key when not in use. On Windows, `VirtualProtect` with `PAGE_NOACCESS`. On macOS, `mprotect` is also available. The key is never written to disk, swap, or any persistent store.

### 11.2 Web (WASM)

Implements Req 23.

The `zoo-wasm` crate wraps `zoo-client` with `#[wasm_bindgen]` exports. The `crypto` crate is compiled separately to WASM for use in the web app.

```rust
// crates/zoo-wasm/src/lib.rs

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ZooHandle {
    client: zoo_client::ZooClient,
}

#[wasm_bindgen]
impl ZooHandle {
    #[wasm_bindgen(constructor)]
    pub async fn create(config: JsValue) -> Result<ZooHandle, JsError>;

    pub async fn upload_file(&self, encrypted_bytes: &[u8], metadata: JsValue)
        -> Result<JsValue, JsError>;

    pub async fn pending_uploads(&self) -> Result<JsValue, JsError>;

    pub async fn cancel_upload(&self, upload_id: &str) -> Result<(), JsError>;

    pub fn close(&self);
}
```

**Build process (Req 23.1):**
```
wasm-pack build crates/zoo-wasm --target web --out-dir apps/web/src/wasm
```

**OpenAPI TypeScript client generation (Req 23.6):**
```
# Generate OpenAPI spec from zoo server
cargo run --bin gen-openapi --manifest-path crates/zoo/Cargo.toml > openapi.json

# Generate TypeScript client
cd packages/api-client && pnpm openapi-typescript ../../openapi.json -o src/generated.ts
```

The `zoo` crate uses `utoipa` to derive OpenAPI annotations from handler types. The TypeScript client in `packages/api-client` is auto-generated and never hand-written (Req 23.6).

**Upload resume in web (Req 23.4, 17.7):** The web client persists `upload_id` in `localStorage`. On page reload, the SSE connection is re-established and the `upload.pending` event delivers the stalled upload state. If a File System Access API handle is available, the user is prompted to resume.

### 11.3 Monorepo Structure

Full directory tree of the polyglot monorepo (Req 22, 23, ARCHITECTURE.md §3):

```
rey/
├── Cargo.toml                      # Virtual workspace manifest (Rust)
├── Cargo.lock
├── pnpm-workspace.yaml             # pnpm workspace config (JS/TS)
├── package.json                    # Root orchestration scripts
├── turbo.json                      # Turborepo task graph
├── Makefile                        # Top-level orchestration
│
├── crates/
│   ├── types/
│   ├── common/
│   ├── crypto/
│   ├── image/
│   ├── metadata/
│   ├── thumbnail/
│   ├── local-db/
│   ├── sync/
│   ├── zoo-client/
│   ├── zoo/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   ├── bin/zoo-server.rs
│   │   └── migrations/
│   ├── client-lib/
│   └── zoo-wasm/
│
├── apps/
│   ├── desktop/                    # Tauri 2.0 app
│   │   ├── src/                    # Frontend (React + shadcn/ui)
│   │   │   ├── app/                # App Router pages
│   │   │   ├── components/         # App-specific components
│   │   │   └── bindings.ts         # Generated by tauri_specta
│   │   ├── src-tauri/              # Tauri Rust side (thin shell)
│   │   │   ├── src/main.rs
│   │   │   └── Cargo.toml          # deps: tauri, client-lib
│   │   ├── package.json
│   │   └── tauri.conf.json
│   │
│   └── web/                        # Next.js web app
│       ├── src/
│       │   ├── app/                # App Router pages
│       │   └── lib/                # Uses packages/api-client
│       ├── next.config.ts
│       └── package.json
│
├── packages/
│   ├── ui/                         # Shared React components (shadcn-based)
│   ├── api-client/                 # Generated TypeScript client from OpenAPI
│   ├── types/                      # Shared TypeScript types
│   ├── tsconfig/                   # Shared TypeScript configs
│   └── eslint-config/              # Shared ESLint configs
│
├── docs/
│   ├── ARCHITECTURE.md
│   ├── SPEC.md
│   ├── STRUCTURE.md
│   └── ZOO.md
│
├── scripts/
│   ├── gen-openapi.sh
│   └── gen-bindings.sh
│
└── .github/
    └── workflows/
        ├── ci.yml                  # cargo test --workspace + pnpm turbo test
        └── release.yml             # Build desktop binaries + deploy web
```


---

## Error Handling

This section covers error types, propagation strategies, and handling across all crates.

### Error Types Per Crate

| Crate | Error type | Key variants |
|-------|-----------|-------------|
| `crypto` | `CryptoError` | `MacMismatch`, `UnsupportedCipher`, `AllocationFailed`, `InvalidKey`, `InvalidNonce` |
| `image` | `ImageError` | `UnsupportedFormat`, `DecodeError`, `ExifError` |
| `metadata` | `MetadataError` | `CryptoError(CryptoError)`, `SerdeError`, `InvalidUtf8` |
| `thumbnail` | `ThumbnailError` | `UnsupportedFormat`, `GenerationFailed`, `CryptoError(CryptoError)`, `CacheError` |
| `local-db` | `LocalDbError` | `KeychainUnavailable`, `InvalidKey`, `MigrationFailed`, `QueryError(rusqlite::Error)` |
| `sync` | `SyncError` | `NetworkError`, `DecryptionFailed { file_id, source }`, `DbError`, `CursorError` |
| `zoo` | `ApiError` | `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `BadRequest`, `InternalError` |
| `zoo-client` | `ZooError` | `HttpError`, `S3Error`, `UploadAborted`, `StateError`, `ParseError` |
| `client-lib` | `CommandError` | `NotLoggedIn`, `SyncError(SyncError)`, `DbError`, `ZooError(ZooError)`, `CryptoError(CryptoError)` |

### Error Propagation Rules

1. **Crypto errors never return partial plaintext.** `CryptoError::MacMismatch` is returned immediately without exposing any decrypted bytes (Req 5.7).
2. **Sync decryption failures are non-fatal.** A `SyncError::DecryptionFailed` for one record is logged and skipped; the sync continues with remaining records (Req 7.7).
3. **Local DB keychain failure is fatal.** If the platform keychain is unavailable, `LocalDbError::KeychainUnavailable` is returned and the DB is not opened in an unencrypted state (Req 9.7).
4. **Upload failures are surfaced via SSE.** When the GC or stall detector marks an upload FAILED, an `upload.failed` SSE event is broadcast to all connected devices (Req 18.4, 16.2).
5. **API errors use a standard JSON schema.** All Zoo API errors return `{"error": {"code": "...", "message": "...", "details": {...}}}` (ZOO.md §17).

### API Error Response Schema (ZOO.md §17)

All Zoo API errors follow this JSON structure:

```json
{
  "error": {
    "code": "error_code_snake_case",
    "message": "Human-readable description",
    "details": {}
  }
}
```

Standard error codes:

| Code | HTTP | Meaning |
|---|---|---|
| `unauthorized` | 401 | Missing or invalid auth |
| `forbidden` | 403 | Authenticated but not authorized for this resource |
| `not_found` | 404 | Upload or file not found |
| `upload_already_exists` | 409 | Active upload already exists for this file_hash |
| `invalid_state_transition` | 409 | Status transition not allowed by state machine |
| `device_name_taken` | 409 | Device name already in use by this user |
| `validation_error` | 400 | Request body validation failed |
| `file_too_large` | 400 | Exceeds max_file_size |
| `part_count_exceeded` | 400 | Exceeds max_part_count |
| `size_mismatch` | 400 | File size doesn't match S3 HeadObject |
| `rate_limited` | 429 | Rate limit exceeded (includes `Retry-After` header) |
| `internal_error` | 500 | Unexpected server error |

### Security Considerations

### 12.1 Zero-Knowledge Guarantee

Implements Req 24. Updated from SPEC.md §5.1 to include all fields in the current design.

| Data | Server visibility | Notes |
|------|------------------|-------|
| File size | **Yes** | Required for S3 storage and HeadObject verification |
| MIME type | **Yes** | Required for CDN content-type headers |
| Content hash (SHA-256 of plaintext) | **Yes** | Required for dedup — known leak (Req 24.6) |
| Encrypted file bytes | **Yes** | Stored in S3 as ciphertext |
| Encrypted metadata blob | **Yes** | Stored in `files.encrypted_metadata` |
| Encrypted thumbnail bytes | **Yes** | Stored in S3 as ciphertext |
| Cipher identifier | **Yes** | Required to know which algorithm to tell client |
| Argon2id parameters (salt, mem, ops) | **Yes** | Required for client to re-derive KEK |
| `bcrypt(VerifyKeyHash)` | **Yes** | Required for login verification |
| Encrypted MasterKey + nonce | **Yes** | Required for client to recover MasterKey |
| Encrypted CollectionKey + nonce | **Yes** | Required for client to decrypt collection |
| Encrypted FileKey + nonce | **Yes** | Required for client to decrypt file |
| X25519 PublicKey | **Yes** | Required for sharing |
| Encrypted X25519 SecretKey | **Yes** | Required for client to decrypt shares |
| File title, description | **No** | Encrypted in metadata blob with FileKey |
| GPS coordinates (latitude, longitude) | **No** | Encrypted in metadata blob with FileKey |
| Capture datetime (taken_at) | **No** | Encrypted in metadata blob with FileKey |
| Device make, model | **No** | Encrypted in metadata blob with FileKey |
| Tags | **No** | Encrypted in metadata blob with FileKey |
| Thumbnail pixels | **No** | Encrypted with FileKey before upload |
| KEK | **No** | Derived client-side from password, never transmitted |
| MasterKey (plaintext) | **No** | Decrypted client-side, held in secret memory |
| CollectionKey (plaintext) | **No** | Decrypted client-side, never transmitted |
| FileKey (plaintext) | **No** | Decrypted client-side, never transmitted |
| VerifyKeyHash (plaintext) | **No** | Only `bcrypt(VerifyKeyHash)` stored |
| Password | **No** | Never transmitted; only derived artifacts |

**Compile-time enforcement (Req 24.3, 24.4):** The `zoo` crate's `Cargo.toml` does not list `crypto`, `image`, `metadata`, or `thumbnail`. The `zoo-wasm` crate's `Cargo.toml` does not list these either. Cargo's dependency resolver enforces this at compile time — any attempt to use these crates in `zoo` or `zoo-wasm` would fail to compile.

### 12.2 Timing Attack Mitigations

Implements Req 2.4, NITPICKS #26.

**Login verification:** The `bcrypt_verify` function uses constant-time comparison internally. Additionally, for non-existent emails, Zoo runs a dummy `bcrypt_verify` against a server-side constant hash to ensure the response time is indistinguishable from a real verification failure:

```rust
// crates/zoo/src/api/auth.rs

async fn login(body: LoginRequest, db: &Db) -> Result<LoginResponse, ApiError> {
    let user = db.find_user_by_email(&body.email).await?;

    // Always run bcrypt_verify, even for non-existent users (Req 2.1, 2.4)
    let stored_hash = match &user {
        Some(u) => u.verify_key_hash.clone(),
        None => DUMMY_BCRYPT_HASH.to_string(),  // constant, never matches
    };

    // constant_time bcrypt comparison (Req 2.4, NITPICKS #26)
    let matches = bcrypt::verify(&body.verify_key_hash, &stored_hash)
        .unwrap_or(false);

    if !matches || user.is_none() {
        return Err(ApiError::Unauthorized);  // Req 2.7: no email hint
    }

    // ... generate session token
}
```

`DUMMY_BCRYPT_HASH` is a pre-computed bcrypt hash of a random string, stored as a server constant. It ensures the timing of a failed login for a non-existent email is identical to a failed login for an existing email.

### 5.9 Rate Limiting

Implements ZOO.md §18.2. Rate limits are enforced via a Tower middleware layer.

| Endpoint | Limit | Window | Response on exceed |
|---|---|---|---|
| POST /api/uploads | 100 | per hour per user | HTTP 429 + `Retry-After` header |
| POST .../presign | 50 | per upload | HTTP 429 + `Retry-After` header |
| PATCH /api/uploads/{id} | 1 per 5 seconds | per upload | HTTP 429 + `Retry-After` header |
| GET /api/files/{id}/download | 1000 | per hour per user | HTTP 429 + `Retry-After` header |
| GET /api/events | 1 concurrent | per device | HTTP 429 + `Retry-After` header |

Rate limit state is stored in-memory using `dashmap::DashMap<String, RateLimitState>` keyed by user_id. For horizontal scaling, a Redis-backed rate limiter can be swapped in later. Rate-limited requests return HTTP 429 with a `Retry-After` header indicating seconds until the window resets.

### 5.10 Input Validation

Implements ZOO.md §18.3. All request bodies are validated before processing using `axum` extractors with custom validation.

| Field | Validation | Error on failure |
|---|---|---|
| file_size | ≤ max_file_size (10 GiB) | 400 `validation_error` |
| part_size | ≥ 5 MiB, ≤ 5 GiB | 400 `validation_error` |
| part_count | ≥ 1, ≤ 10000 | 400 `part_count_exceeded` |
| part_md5s | must match part_count, each must decode to 16 bytes | 400 `validation_error` |
| email | valid email format, ≤ 255 chars | 400 `validation_error` |
| device name | ≤ 64 chars, no null bytes | 400 `validation_error` |
| All string fields | length-limited, Unicode-safe, no null bytes | 400 `validation_error` |

Validation is performed in the Axum handler before any DB or S3 operations. Invalid requests return HTTP 400 with the standard error schema (§12.5).

### 5.11 Configuration

Implements ZOO.md §15. Environment-driven configuration loaded at startup.

```rust
// crates/zoo/src/config.rs

pub struct ZooConfig {
    pub listen_addr: SocketAddr,              // default: 0.0.0.0:3002
    pub database_url: String,                 // PostgreSQL connection string
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub session_ttl: Duration,                // default: 30 days
    pub download_mode: DownloadMode,          // redirect or proxy
    pub stall_timeout: Duration,              // default: 90s
    pub presigned_ttl: Duration,              // default: 24h
    pub gc_interval: Duration,                // default: 5m
    pub max_file_size: u64,                   // default: 10GiB
    pub max_part_count: u16,                  // default: 10000
    pub default_part_size: u32,               // default: 20MiB
}

pub enum DownloadMode {
    Redirect { presigned_ttl: Duration },
    Proxy { max_concurrent: usize },
}
```

Environment variable mapping:

| Env Var | Config Field | Default |
|---|---|---|
| `LISTEN_ADDR` | `listen_addr` | `0.0.0.0:3002` |
| `DATABASE_URL` | `database_url` | — (required) |
| `S3_ENDPOINT` | `s3_endpoint` | — (required) |
| `S3_REGION` | `s3_region` | — (required) |
| `S3_BUCKET` | `s3_bucket` | — (required) |
| `S3_ACCESS_KEY` | `s3_access_key` | — (required) |
| `S3_SECRET_KEY` | `s3_secret_key` | — (required) |
| `SESSION_TTL_DAYS` | `session_ttl` | `30` |
| `DOWNLOAD_MODE` | `download_mode` | `redirect` |
| `STALL_TIMEOUT_SECONDS` | `stall_timeout` | `90` |
| `PRESIGNED_TTL_HOURS` | `presigned_ttl` | `24` |
| `GC_INTERVAL_SECONDS` | `gc_interval` | `300` |
| `MAX_FILE_SIZE` | `max_file_size` | `10737418240` |
| `DEFAULT_PART_SIZE` | `default_part_size` | `20971520` |

### 12.3 Memory Protection

Implements Req 2.11, 22.7.

The `Key256` type uses the `zeroize` crate:

```rust
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Key256([u8; 32]);
```

`ZeroizeOnDrop` ensures the key bytes are overwritten with zeros when the `Key256` value is dropped, preventing the key from lingering in freed memory.

For additional protection, the `AppState.master_key` field is wrapped in a `SecretBox` (from the `secrecy` crate) which applies OS-level memory protection:

- **Linux:** `mprotect(PROT_NONE)` on the memory page when not in use; `mprotect(PROT_READ|PROT_WRITE)` when accessed.
- **Windows:** `VirtualProtect(PAGE_NOACCESS)` / `PAGE_READWRITE`.
- **macOS:** `mprotect` (same as Linux).

The MasterKey is never written to disk, swap, or any persistent store (Req 2.11, 22.7). It is re-derived from the password on each login.

### 12.4 Known Limitations

These are accepted tradeoffs documented in SPEC.md §5.1 and Req 24.6:

1. **Content hash leak (Req 24.6):** `SHA-256(plaintext)` is stored in `files.content_hash` for deduplication. If two users upload the same file, the server learns they have identical content. Mitigation (future): keyed hashes or blind deduplication. Not in v1 scope.

2. **Local DB decrypted metadata:** The `local-db` SQLite database stores decrypted file metadata (titles, locations, dates) on the trusted device. If the device is physically compromised, this metadata is readable. This is an accepted tradeoff: privacy during transit vs. usability on the local device. The DB is encrypted at rest with a platform keychain key (Req 9.6), but the key is accessible to the OS user.

3. **MIME type and file size visibility:** The server knows the MIME type and file size of every file. This is required for S3 storage and CDN delivery. It reveals the general nature of files (photo vs. video) and their approximate size.

4. **Timing of uploads:** The server knows when files are uploaded and by which device. Upload timestamps are not encrypted.

5. **Collection structure:** The server knows how many collections a user has and how many files are in each collection (from the `files` table). The collection names and file metadata are encrypted, but the structure is visible.


---

## Correctness Properties

Property-based tests and invariants that must hold across the system.

### Crypto Round-Trip Properties

The following properties are verified with `proptest` or `quickcheck`:

| Property | Crate | Requirement |
|----------|-------|-------------|
| `∀ key, plaintext: decrypt(encrypt(plaintext, key), key) == plaintext` | `crypto` | Req 5.8 |
| `∀ key, thumbnail: decrypt(encrypt(thumbnail, key), key) == thumbnail` | `thumbnail` | Req 10.7 |
| `∀ key, metadata: decrypt(encrypt(metadata, key), key) == metadata` | `metadata` | Req 5.2 |
| `∀ modified_ciphertext: decrypt(modified_ciphertext, key) == Err(MacMismatch)` | `crypto` | Req 5.6, 5.7 |
| `∀ n, rows: paginate(rows, n).last_group_not_split == true` | `zoo` | Req 7.3 |
| `∀ bitmask: encode(decode(bitmask)) == bitmask` | `types` | Req 13.5 |

### State Machine Invariants

- An upload in state DONE can never transition to any other state.
- An upload in state FAILED can never transition to any other state.
- STALLED uploads can only resume to UPLOADING (never ENCRYPTING), because encryption completed before the stall (Req 13.3).
- The `parts_bitmask` bit count never decreases (parts are never un-uploaded).
- `expires_at` is always set and always in the future for non-terminal states.

### Zero-Knowledge Invariants

- The `zoo` crate binary must not contain any symbol from `crypto`, `image`, `metadata`, or `thumbnail` crates (verified by `cargo tree --no-dedupe`).
- The `zoo-wasm` binary must not contain any symbol from `crypto`, `image`, `metadata`, or `thumbnail` crates.
- No plaintext key material appears in any HTTP request or response body (verified by integration tests that inspect all wire traffic).

---

## Testing Strategy

Per-crate test approach, expanded from STRUCTURE.md §9 with specific test cases for each requirement.

| Crate | Test approach | Specific test cases | Requirements covered |
|-------|--------------|--------------------|--------------------|
| `types` | Unit | Serde round-trips for all structs; `UploadStatus` serialization matches snake_case strings; `SseEvent` tag field serialization | Req 25.2 |
| `crypto` | Unit + known-answer vectors | Argon2id KAT against libsodium reference vectors; XSalsa20-Poly1305 encrypt/decrypt round-trip; XChaCha20-Poly1305 encrypt/decrypt round-trip; MAC mismatch returns `CryptoError::MacMismatch` and no partial plaintext; adaptive fallback halves memory and doubles ops; fallback stops at 32 MiB floor; BLAKE2b-KDF VerificationKey derivation matches reference; X25519 sealed box round-trip | Req 1.1–1.8, 3.1–3.6, 5.6–5.8, 6.1–6.3 |
| `image` | Unit + fixtures | JPEG/PNG/WebP decode; EXIF GPS extraction from fixture; EXIF orientation correction (all 8 orientations); resize to max 720px preserves aspect ratio; missing EXIF returns partial result without error | Req 26.1–26.4 |
| `metadata` | Unit | `FileMetadata` encrypt/decrypt round-trip with known FileKey; missing fields serialize as null; JSON round-trip | Req 5.2, 26.3, 26.5 |
| `thumbnail` | Unit + fixtures | Generate thumbnail from JPEG: output ≤ 720px, ≤ 100 KB; iterative quality reduction triggers when output > 100 KB; EXIF orientation applied before encode; encrypt/decrypt round-trip produces identical bytes; unsupported format returns error without panicking | Req 10.1–10.8 |
| `common` | Unit | Config parsing from env vars; error formatting; tracing init | — |
| `local-db` | Integration (tmp SQLite) | Open DB with SQLCipher key; migrations run in order; collections CRUD; files upsert and archive; sync_state read/write; FTS5 text search returns correct results; FTS5 index rebuild; date range query; geographic bounding box query; keychain unavailable returns error without opening unencrypted DB | Req 9.1–9.7, 21.1–21.6 |
| `sync` | Integration (mock HTTP) | `sync_all()` fetches collections then files then trash; cursor persisted after each page; `has_more=true` triggers next page fetch; decryption failure skips record and continues; version-consistent pagination discards incomplete last group; offline mode serves from local DB | Req 7.1–7.7, 8.1–8.8 |
| `zoo` | Integration (test DB + MinIO) | Registration stores `bcrypt(VerifyKeyHash)`, not plaintext; login with correct hash returns session token; login with wrong hash returns 401; login for non-existent email returns 401 (no timing difference); state machine rejects invalid transitions with 400; STALLED→UPLOADING allowed, STALLED→ENCRYPTING rejected; parts_bitmask stored as big-endian bit vector; stall detector marks UPLOADING→STALLED after timeout; stall detector uses SKIP LOCKED; GC marks expired uploads FAILED and calls S3 abort; GC uses SKIP LOCKED; SSE hub broadcasts to all subscribers; Postgres LISTEN/NOTIFY fan-out; registration is idempotent (same upload_id returns same file_id); HeadObject size mismatch returns 400; dedup index prevents duplicate active uploads for same file+collection | Req 1.9–1.12, 2.1–2.10, 12–19, 24.3 |
| `zoo-client` | Integration (mock server) | Full upload flow: POST→PATCH→presign→PUT parts→PATCH s3_completed→POST register; heartbeat sent every 30s; 403 from S3 triggers presign-refresh and retry; resume from STALLED transitions to UPLOADING (not ENCRYPTING); S3 ListParts reconciliation: all 5 cases in table; `NoSuchUpload` marks FAILED; upload_id persisted in state store | Req 13.3, 14.1–14.6, 17.1–17.7 |
| `client-lib` | Integration (mock deps) | `login` command decrypts MasterKey and stores in AppState; `upload_file` command encrypts file and calls zoo-client; `get_thumbnail` command returns from memory cache on hit; `search_files` command executes FTS5 query; `trigger_sync` command calls sync_all; MasterKey not present in AppState after logout | Req 22.2–22.7 |

### Integration Test Infrastructure

- **Zoo tests:** Use `sqlx::test` macro with a real PostgreSQL instance (Docker). Use `minio` (Docker) for S3 tests.
- **local-db tests:** Use `tempfile::TempDir` for isolated SQLite databases per test.
- **sync tests:** Use `wiremock` to mock Zoo HTTP responses.
- **zoo-client tests:** Use `wiremock` for Zoo mock + `mockito` for S3 mock.

### CI Pipeline

```yaml
# .github/workflows/ci.yml
jobs:
  rust:
    steps:
      - cargo test --workspace --all-features
      - cargo clippy --workspace -- -D warnings
      - cargo fmt --check
  js:
    steps:
      - pnpm turbo test
      - pnpm turbo lint
      - pnpm turbo typecheck
```

The `cargo test --workspace` command runs all crate tests in dependency order. The `--all-features` flag ensures feature-gated code paths are tested.
