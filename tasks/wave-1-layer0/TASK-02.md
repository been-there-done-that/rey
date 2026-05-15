# Task 2: Implement `crates/types` — Layer 0 Shared Data Types

## Wave
1 (Layer 0 — Foundation types)

## Dependencies
- Task 1 (Scaffold workspace) must be complete
- `Cargo.toml` virtual manifest must exist with `[workspace.dependencies]`

## Can Run In Parallel With
- Task 3 (common crate) — no dependencies between types and common

## Design References
- design.md §4.1: Types Crate Module Structure
- design.md §4.2: Core Types (FileMetadata, EncryptedFileRecord, KeyAttributes, UploadStatus, SseEvent, SyncCollectionResponse, SyncFilesResponse, SyncTrashResponse, DeletedFileRef)
- design.md §3.3: Key Types (Key256, Nonce24, Header24, Salt16, EncryptedKey, Argon2Profile)

## Requirements
25.2, 1.1–1.8, 3.5–3.6, 4.7, 5.2, 8.2, 9.2, 4.1, 7.2, 7.3, 8.7, 13.1, 13.5, 28.4, 19.4, 19.5, 12.1, 6.4, 2.5

## Objective
Implement all shared data types with `serde` derives. Zero logic. This crate ships in both client and server.

## Cargo.toml
```toml
[package]
name = "types"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
zeroize = { workspace = true }
```

## Files to Create

### `src/lib.rs`
Re-export all public types from submodules:
```rust
pub mod crypto;
pub mod file;
pub mod collection;
pub mod sync;
pub mod upload;
pub mod sse;
pub mod device;
pub mod share;
pub mod user;
pub mod error;
```

### `src/crypto.rs`
Define:
- `Key256` — newtype wrapper around `[u8; 32]`, derive `Clone`, `Debug`, `Serialize`, `Deserialize`. Add `Zeroize`, `ZeroizeOnDrop` derives (from `zeroize` crate, add to deps).
- `Nonce24` — newtype around `[u8; 24]`, derive `Clone`, `Copy`, `Debug`, `Serialize`, `Deserialize`.
- `Header24` — newtype around `[u8; 24]`, derive `Clone`, `Copy`, `Debug`, `Serialize`, `Deserialize`.
- `Salt16` — newtype around `[u8; 16]`, derive `Clone`, `Copy`, `Debug`, `Serialize`, `Deserialize`.
- `EncryptedKey` — struct with `nonce: Nonce24`, `ciphertext: Vec<u8>` (MAC 16 bytes || ciphertext). Derive `Clone`, `Debug`, `Serialize`, `Deserialize`.
- `KeyAttributes` — struct with `encrypted_master_key: String` (base64), `key_nonce: String`, `kek_salt: String`, `mem_limit: u32`, `ops_limit: u32`. Derive `Clone`, `Debug`, `Serialize`, `Deserialize`.
- `Argon2Profile` — enum with `Sensitive`, `Mobile`, `Interactive`. Derive `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`. Implement `mem_limit()` returning 256 MiB / 128 MiB / 64 MiB and `ops_limit()` returning 4 / 3 / 2.

#### Implementation Details for `src/crypto.rs`

```rust
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 256-bit cryptographic key material.
///
/// Intentionally does NOT implement `Default` or `PartialEq` to prevent
/// accidental comparison of key material.
#[derive(Clone, Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct Key256([u8; 32]);

impl Key256 {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// 24-byte nonce for XChaCha20-Poly1305.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nonce24([u8; 24]);

impl Nonce24 {
    pub fn new(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

/// 24-byte header for libsodium secretbox-style encryption.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Header24([u8; 24]);

impl Header24 {
    pub fn new(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

/// 16-byte salt for Argon2 key derivation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Salt16([u8; 16]);

impl Salt16 {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// An encrypted key with its associated nonce.
///
/// The `ciphertext` field contains MAC (16 bytes) || ciphertext.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedKey {
    pub nonce: Nonce24,
    pub ciphertext: Vec<u8>,
}

/// Key attributes stored on the server for login/key derivation.
///
/// All byte fields are base64-encoded strings for JSON transport.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyAttributes {
    pub encrypted_master_key: String,
    pub key_nonce: String,
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
}

/// Argon2id memory/ops profile for different use cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argon2Profile {
    /// For master key derivation: 256 MiB, 4 ops
    Sensitive,
    /// For mobile devices: 128 MiB, 3 ops
    Mobile,
    /// For interactive/key verification: 64 MiB, 2 ops
    Interactive,
}

impl Argon2Profile {
    /// Returns memory limit in bytes.
    pub fn mem_limit(&self) -> u32 {
        match self {
            Argon2Profile::Sensitive => 256 * 1024 * 1024,
            Argon2Profile::Mobile => 128 * 1024 * 1024,
            Argon2Profile::Interactive => 64 * 1024 * 1024,
        }
    }

    /// Returns ops limit (iterations).
    pub fn ops_limit(&self) -> u32 {
        match self {
            Argon2Profile::Sensitive => 4,
            Argon2Profile::Mobile => 3,
            Argon2Profile::Interactive => 2,
        }
    }
}
```

### `src/file.rs`
Define:
- `FileMetadata` — struct with `title: Option<String>`, `description: Option<String>`, `latitude: Option<f64>`, `longitude: Option<f64>`, `taken_at: Option<i64>` (Unix ms), `device_make: Option<String>`, `device_model: Option<String>`, `tags: Vec<String>`. Derive `Debug`, `Clone`, `Serialize`, `Deserialize`.
- `EncryptedFileRecord` — struct with `id: i64`, `collection_id: String`, `cipher: String` (default "xchacha20-poly1305"), `encrypted_key: String`, `key_decryption_nonce: String`, `file_decryption_header: String`, `thumb_decryption_header: Option<String>`, `encrypted_metadata: String`, `encrypted_thumbnail: Option<String>`, `thumbnail_size: Option<i32>`, `file_size: i64`, `mime_type: String`, `content_hash: String`, `object_key: String`, `updation_time: i64`, `created_at: i64`, `archived_at: Option<i64>`. Derive `Debug`, `Clone`, `Serialize`, `Deserialize`.
- `FileRecord` — the decrypted version: `id: i64`, `collection_id: String`, `cipher: String`, `title: Option<String>`, `description: Option<String>`, `latitude: Option<f64>`, `longitude: Option<f64>`, `taken_at: Option<i64>`, `file_size: i64`, `mime_type: String`, `content_hash: String`, `encrypted_key: String`, `key_nonce: String`, `file_decryption_header: String`, `thumb_decryption_header: Option<String>`, `object_key: String`, `thumbnail_path: Option<String>`, `updation_time: i64`, `created_at: i64`, `archived_at: Option<i64>`.

#### Implementation Details for `src/file.rs`

```rust
use serde::{Deserialize, Serialize};

/// User-editable metadata for a file (before encryption).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,
    pub device_make: Option<String>,
    pub device_model: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Encrypted file record as stored on the server.
///
/// All sensitive fields are encrypted and stored as base64 strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFileRecord {
    pub id: i64,
    pub collection_id: String,
    #[serde(default = "default_cipher")]
    pub cipher: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub encrypted_metadata: String,
    pub encrypted_thumbnail: Option<String>,
    pub thumbnail_size: Option<i32>,
    pub file_size: i64,
    pub mime_type: String,
    pub content_hash: String,
    pub object_key: String,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

fn default_cipher() -> String {
    "xchacha20-poly1305".to_string()
}

/// Decrypted file record after client-side decryption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub collection_id: String,
    pub cipher: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,
    pub file_size: i64,
    pub mime_type: String,
    pub content_hash: String,
    pub encrypted_key: String,
    pub key_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub object_key: String,
    pub thumbnail_path: Option<String>,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}
```

### `src/collection.rs`
Define:
- `Collection` — struct with `id: String`, `name: String` (decrypted), `encrypted_key: String`, `key_nonce: String`, `updation_time: i64`, `created_at: i64`, `archived_at: Option<i64>`.
- `EncryptedCollection` — struct with `id: String`, `encrypted_name: String`, `name_decryption_nonce: String`, `encrypted_key: String`, `key_decryption_nonce: String`, `updation_time: i64`.

#### Implementation Details for `src/collection.rs`

```rust
use serde::{Deserialize, Serialize};

/// A decrypted collection (folder/album).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub encrypted_key: String,
    pub key_nonce: String,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

/// Encrypted collection as stored on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCollection {
    pub id: String,
    pub encrypted_name: String,
    pub name_decryption_nonce: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub updation_time: i64,
}
```

### `src/sync.rs`
Define:
- `SyncCollectionResponse` — `collections: Vec<EncryptedCollection>`, `has_more: bool`, `latest_updated_at: i64`.
- `SyncFilesResponse` — `updated_files: Vec<EncryptedFileRecord>`, `deleted_file_ids: Vec<i64>`, `has_more: bool`, `latest_updated_at: i64`.
- `SyncTrashResponse` — `deleted_files: Vec<DeletedFileRef>`, `has_more: bool`, `latest_updated_at: i64`.
- `DeletedFileRef` — `file_id: i64`, `collection_id: String`, `updation_time: i64`.
- `SyncCursor` — `key: String`, `value: i64`.

#### Implementation Details for `src/sync.rs`

```rust
use serde::{Deserialize, Serialize};
use crate::collection::EncryptedCollection;
use crate::file::EncryptedFileRecord;

/// Response from syncing collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCollectionResponse {
    pub collections: Vec<EncryptedCollection>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

/// Response from syncing files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFilesResponse {
    pub updated_files: Vec<EncryptedFileRecord>,
    pub deleted_file_ids: Vec<i64>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

/// Response from syncing trash/deleted files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTrashResponse {
    pub deleted_files: Vec<DeletedFileRef>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

/// Reference to a deleted file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFileRef {
    pub file_id: i64,
    pub collection_id: String,
    pub updation_time: i64,
}

/// Cursor for pagination in sync requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCursor {
    pub key: String,
    pub value: i64,
}
```

### `src/upload.rs`
Define:
- `UploadStatus` — enum with `Pending`, `Encrypting`, `Uploading`, `S3Completed`, `Registering`, `Done`, `Stalled`, `Failed`. Derive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`. Add `#[serde(rename_all = "snake_case")]`.
- `UploadState` — struct with `upload_id: String` (UUID), `user_id: String`, `device_id: String`, `status: UploadStatus`, `file_hash: String`, `file_size: i64`, `mime_type: Option<String>`, `part_size: i32`, `part_count: u16`, `parts_bitmask: String` (base64), `object_key: Option<String>`, `upload_id_s3: Option<String>`, `complete_url: Option<String>`, `urls_expire_at: Option<i64>`, `last_heartbeat_at: Option<i64>`, `stalled_at: Option<i64>`, `error_reason: Option<String>`, `created_at: i64`, `expires_at: i64`, `done_at: Option<i64>`.
- `PartRecord` — `part_number: u16`, `part_size: i32`, `part_md5: String`, `etag: Option<String>`, `status: String` ("pending" or "uploaded").
- `UploadSummary` — `upload_id: String`, `status: UploadStatus`, `file_hash: String`, `file_size: i64`, `part_count: u16`, `parts_completed: u16`, `device_name: String`, `stalled_at: Option<i64>`.

#### Implementation Details for `src/upload.rs`

```rust
use serde::{Deserialize, Serialize};

/// Status of a file upload through its lifecycle.
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

/// Full state of an upload tracked by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadState {
    pub upload_id: String,
    pub user_id: String,
    pub device_id: String,
    pub status: UploadStatus,
    pub file_hash: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub part_size: i32,
    pub part_count: u16,
    pub parts_bitmask: String,
    pub object_key: Option<String>,
    pub upload_id_s3: Option<String>,
    pub complete_url: Option<String>,
    pub urls_expire_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub stalled_at: Option<i64>,
    pub error_reason: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub done_at: Option<i64>,
}

/// Record for a single multipart upload part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartRecord {
    pub part_number: u16,
    pub part_size: i32,
    pub part_md5: String,
    pub etag: Option<String>,
    pub status: String,
}

/// Summary of an upload for display/SSE purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSummary {
    pub upload_id: String,
    pub status: UploadStatus,
    pub file_hash: String,
    pub file_size: i64,
    pub part_count: u16,
    pub parts_completed: u16,
    pub device_name: String,
    pub stalled_at: Option<i64>,
}
```

### `src/sse.rs`
Define `SseEvent` enum with `#[serde(tag = "type", rename_all = "snake_case")]`:
- `UploadProgress { upload_id, status: UploadStatus, parts_bitmask: String, part_count: u16, device_name: String }`
- `UploadCompleted { upload_id, device_name }`
- `UploadDone { upload_id, file_id: i64, device_name }`
- `UploadStalled { upload_id, parts_bitmask, part_count, device_name, stalled_at: i64 }`
- `UploadFailed { upload_id, reason, device_name }`
- `UploadPending { uploads: Vec<UploadSummary> }`
- `DeviceConnected { device_id, device_name }`
- `DeviceDisconnected { device_id, device_name }`
- `Heartbeat { timestamp: i64 }`

#### Implementation Details for `src/sse.rs`

```rust
use serde::{Deserialize, Serialize};
use crate::upload::{UploadStatus, UploadSummary};

/// Server-sent event types for real-time communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    UploadProgress {
        upload_id: String,
        status: UploadStatus,
        parts_bitmask: String,
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
```

### `src/device.rs`
Define:
- `DeviceInfo` — `device_id: String`, `name: String`, `platform: DevicePlatform`, `sse_token: String`, `push_token: Option<String>`, `stall_timeout_seconds: u32`.
- `DeviceRegistration` — `name: String`, `platform: DevicePlatform`, `push_token: Option<String>`.
- `DevicePlatform` — enum: `Android`, `Ios`, `Web`, `Desktop`. Derive `Serialize`, `Deserialize`, `Clone`, `Debug`.

#### Implementation Details for `src/device.rs`

```rust
use serde::{Deserialize, Serialize};

/// Platform identifier for a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevicePlatform {
    Android,
    Ios,
    Web,
    Desktop,
}

/// Full information about a registered device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: String,
    pub platform: DevicePlatform,
    pub sse_token: String,
    pub push_token: Option<String>,
    pub stall_timeout_seconds: u32,
}

/// Request to register a new device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub name: String,
    pub platform: DevicePlatform,
    pub push_token: Option<String>,
}
```

### `src/share.rs`
Define:
- `ShareRecord` — `file_id: i64`, `shared_with: String` (user_id), `collection_id: String`, `encrypted_collection_key: String`, `created_at: i64`, `expires_at: Option<i64>`.
- `ShareRequest` — `file_id: i64`, `shared_with: String`, `collection_id: String`, `encrypted_collection_key: String`, `expires_at: Option<i64>`.

#### Implementation Details for `src/share.rs`

```rust
use serde::{Deserialize, Serialize};

/// A share record stored on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    pub file_id: i64,
    pub shared_with: String,
    pub collection_id: String,
    pub encrypted_collection_key: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

/// Request to create a new share.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRequest {
    pub file_id: i64,
    pub shared_with: String,
    pub collection_id: String,
    pub encrypted_collection_key: String,
    pub expires_at: Option<i64>,
}
```

### `src/user.rs`
Define:
- `UserRegistration` — `email: String`, `verify_key_hash: String`, `encrypted_master_key: String`, `key_nonce: String`, `kek_salt: String`, `mem_limit: u32`, `ops_limit: u32`, `public_key: String`, `encrypted_secret_key: String`, `secret_key_nonce: String`, `encrypted_recovery_key: String`, `recovery_key_nonce: String`.
- `LoginParams` — `kek_salt: String`, `mem_limit: u32`, `ops_limit: u32`.
- `LoginRequest` — `email: String`, `verify_key_hash: String`.
- `LoginResponse` — `session_token: String`, `key_attributes: KeyAttributes`.
- `SessionInfo` — `user_id: String`, `expires_at: i64`.

#### Implementation Details for `src/user.rs`

```rust
use serde::{Deserialize, Serialize};
use crate::crypto::KeyAttributes;

/// Registration payload for a new user.
///
/// All cryptographic material is pre-encrypted client-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegistration {
    pub email: String,
    pub verify_key_hash: String,
    pub encrypted_master_key: String,
    pub key_nonce: String,
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
    pub public_key: String,
    pub encrypted_secret_key: String,
    pub secret_key_nonce: String,
    pub encrypted_recovery_key: String,
    pub recovery_key_nonce: String,
}

/// Parameters needed to derive the KEK during login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginParams {
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
}

/// Login request with email and verification key hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub verify_key_hash: String,
}

/// Response after successful login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub session_token: String,
    pub key_attributes: KeyAttributes,
}

/// Session metadata for middleware/auth checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub user_id: String,
    pub expires_at: i64,
}
```

### `src/error.rs`
Define:
- `ErrorCode` — enum: `Unauthorized`, `Forbidden`, `NotFound`, `UploadAlreadyExists`, `InvalidStateTransition`, `DeviceNameTaken`, `ValidationError`, `FileTooLarge`, `PartCountExceeded`, `SizeMismatch`, `RateLimited`, `InternalError`. Derive `Serialize`, `Deserialize`, `Clone`, `Debug`.
- `ErrorResponse` — `error: ApiError`.
- `ApiError` — `code: ErrorCode`, `message: String`, `details: Option<serde_json::Value>`.

#### Implementation Details for `src/error.rs`

```rust
use serde::{Deserialize, Serialize};

/// Standardized error codes for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    NotFound,
    UploadAlreadyExists,
    InvalidStateTransition,
    DeviceNameTaken,
    ValidationError,
    FileTooLarge,
    PartCountExceeded,
    SizeMismatch,
    RateLimited,
    InternalError,
}

/// A single API error with code, message, and optional details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Top-level error response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}
```

## Tests (Task 2.9 — marked with *)
Write unit tests for:
- Serde round-trips for ALL structs (serialize then deserialize equals original)
- `UploadStatus` serializes to snake_case strings (`"pending"`, `"encrypting"`, etc.)
- `SseEvent` tag field serialization (`{"type":"upload_progress",...}`)
- `Argon2Profile::mem_limit()` returns correct values (256*1024*1024, 128*1024*1024, 64*1024*1024)
- `Argon2Profile::ops_limit()` returns correct values (4, 3, 2)

### Test Implementation Details

Add `#[cfg(test)]` modules at the bottom of each source file:

#### `src/crypto.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_profile_mem_limit_sensitive() {
        assert_eq!(Argon2Profile::Sensitive.mem_limit(), 256 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_mem_limit_mobile() {
        assert_eq!(Argon2Profile::Mobile.mem_limit(), 128 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_mem_limit_interactive() {
        assert_eq!(Argon2Profile::Interactive.mem_limit(), 64 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_ops_limit_sensitive() {
        assert_eq!(Argon2Profile::Sensitive.ops_limit(), 4);
    }

    #[test]
    fn test_argon2_profile_ops_limit_mobile() {
        assert_eq!(Argon2Profile::Mobile.ops_limit(), 3);
    }

    #[test]
    fn test_argon2_profile_ops_limit_interactive() {
        assert_eq!(Argon2Profile::Interactive.ops_limit(), 2);
    }

    #[test]
    fn test_key_attributes_roundtrip() {
        let ka = KeyAttributes {
            encrypted_master_key: "base64key".to_string(),
            key_nonce: "nonce123".to_string(),
            kek_salt: "salt456".to_string(),
            mem_limit: 256 * 1024 * 1024,
            ops_limit: 4,
        };
        let json = serde_json::to_string(&ka).unwrap();
        let decoded: KeyAttributes = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.encrypted_master_key, ka.encrypted_master_key);
        assert_eq!(decoded.mem_limit, ka.mem_limit);
    }

    #[test]
    fn test_encrypted_key_roundtrip() {
        let ek = EncryptedKey {
            nonce: Nonce24::new([0u8; 24]),
            ciphertext: vec![1, 2, 3, 4],
        };
        let json = serde_json::to_string(&ek).unwrap();
        let decoded: EncryptedKey = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.ciphertext, ek.ciphertext);
    }
}
```

#### `src/file.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metadata_roundtrip() {
        let fm = FileMetadata {
            title: Some("test.jpg".to_string()),
            description: Some("A test file".to_string()),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            taken_at: Some(1700000000000),
            device_make: Some("Apple".to_string()),
            device_model: Some("iPhone 15".to_string()),
            tags: vec!["vacation".to_string()],
        };
        let json = serde_json::to_string(&fm).unwrap();
        let decoded: FileMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.title, fm.title);
        assert_eq!(decoded.tags, fm.tags);
    }

    #[test]
    fn test_encrypted_file_record_roundtrip() {
        let efr = EncryptedFileRecord {
            id: 1,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            encrypted_key: "enc_key".to_string(),
            key_decryption_nonce: "nonce".to_string(),
            file_decryption_header: "header".to_string(),
            thumb_decryption_header: None,
            encrypted_metadata: "meta".to_string(),
            encrypted_thumbnail: None,
            thumbnail_size: None,
            file_size: 1024,
            mime_type: "image/jpeg".to_string(),
            content_hash: "hash123".to_string(),
            object_key: "obj/key".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&efr).unwrap();
        let decoded: EncryptedFileRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, efr.id);
        assert_eq!(decoded.file_size, efr.file_size);
    }

    #[test]
    fn test_file_record_roundtrip() {
        let fr = FileRecord {
            id: 1,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            title: Some("test.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            file_size: 2048,
            mime_type: "image/png".to_string(),
            content_hash: "hash".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            file_decryption_header: "fdh".to_string(),
            thumb_decryption_header: None,
            object_key: "ok".to_string(),
            thumbnail_path: Some("/tmp/thumb".to_string()),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&fr).unwrap();
        let decoded: FileRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.title, fr.title);
    }
}
```

#### `src/collection.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_roundtrip() {
        let c = Collection {
            id: "col-1".to_string(),
            name: "Vacation Photos".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let decoded: Collection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, c.name);
    }

    #[test]
    fn test_encrypted_collection_roundtrip() {
        let ec = EncryptedCollection {
            id: "col-1".to_string(),
            encrypted_name: "encrypted_name".to_string(),
            name_decryption_nonce: "nonce".to_string(),
            encrypted_key: "ek".to_string(),
            key_decryption_nonce: "kdn".to_string(),
            updation_time: 1700000000000,
        };
        let json = serde_json::to_string(&ec).unwrap();
        let decoded: EncryptedCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.encrypted_name, ec.encrypted_name);
    }
}
```

#### `src/sync.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::EncryptedCollection;

    #[test]
    fn test_sync_collection_response_roundtrip() {
        let resp = SyncCollectionResponse {
            collections: vec![EncryptedCollection {
                id: "col-1".to_string(),
                encrypted_name: "enc".to_string(),
                name_decryption_nonce: "n".to_string(),
                encrypted_key: "ek".to_string(),
                key_decryption_nonce: "kdn".to_string(),
                updation_time: 1700000000000,
            }],
            has_more: false,
            latest_updated_at: 1700000000000,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: SyncCollectionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.has_more, resp.has_more);
        assert_eq!(decoded.collections.len(), 1);
    }

    #[test]
    fn test_deleted_file_ref_roundtrip() {
        let dfr = DeletedFileRef {
            file_id: 42,
            collection_id: "col-1".to_string(),
            updation_time: 1700000000000,
        };
        let json = serde_json::to_string(&dfr).unwrap();
        let decoded: DeletedFileRef = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.file_id, dfr.file_id);
    }

    #[test]
    fn test_sync_cursor_roundtrip() {
        let cursor = SyncCursor {
            key: "updated_at".to_string(),
            value: 1700000000000,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        let decoded: SyncCursor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.key, cursor.key);
    }
}
```

#### `src/upload.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_status_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&UploadStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Encrypting).unwrap(), "\"encrypting\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Uploading).unwrap(), "\"uploading\"");
        assert_eq!(serde_json::to_string(&UploadStatus::S3Completed).unwrap(), "\"s3_completed\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Registering).unwrap(), "\"registering\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Done).unwrap(), "\"done\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Stalled).unwrap(), "\"stalled\"");
        assert_eq!(serde_json::to_string(&UploadStatus::Failed).unwrap(), "\"failed\"");
    }

    #[test]
    fn test_upload_status_deserializes_from_snake_case() {
        assert_eq!(serde_json::from_str::<UploadStatus>("\"pending\"").unwrap(), UploadStatus::Pending);
        assert_eq!(serde_json::from_str::<UploadStatus>("\"s3_completed\"").unwrap(), UploadStatus::S3Completed);
    }

    #[test]
    fn test_upload_state_roundtrip() {
        let us = UploadState {
            upload_id: "uuid-1".to_string(),
            user_id: "user-1".to_string(),
            device_id: "dev-1".to_string(),
            status: UploadStatus::Uploading,
            file_hash: "hash".to_string(),
            file_size: 1024,
            mime_type: Some("image/jpeg".to_string()),
            part_size: 5 * 1024 * 1024,
            part_count: 4,
            parts_bitmask: "AAAA".to_string(),
            object_key: Some("obj/key".to_string()),
            upload_id_s3: Some("s3-id".to_string()),
            complete_url: Some("http://example.com/complete".to_string()),
            urls_expire_at: Some(1700000000000),
            last_heartbeat_at: Some(1700000000000),
            stalled_at: None,
            error_reason: None,
            created_at: 1700000000000,
            expires_at: 1700003600000,
            done_at: None,
        };
        let json = serde_json::to_string(&us).unwrap();
        let decoded: UploadState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, us.status);
        assert_eq!(decoded.part_count, us.part_count);
    }

    #[test]
    fn test_part_record_roundtrip() {
        let pr = PartRecord {
            part_number: 1,
            part_size: 5242880,
            part_md5: "abc123".to_string(),
            etag: Some("etag456".to_string()),
            status: "uploaded".to_string(),
        };
        let json = serde_json::to_string(&pr).unwrap();
        let decoded: PartRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.part_number, pr.part_number);
    }

    #[test]
    fn test_upload_summary_roundtrip() {
        let summary = UploadSummary {
            upload_id: "uuid-1".to_string(),
            status: UploadStatus::Done,
            file_hash: "hash".to_string(),
            file_size: 1024,
            part_count: 2,
            parts_completed: 2,
            device_name: "My Phone".to_string(),
            stalled_at: None,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let decoded: UploadSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, summary.status);
    }
}
```

#### `src/sse.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::upload::UploadStatus;

    #[test]
    fn test_sse_event_upload_progress_tag() {
        let event = SseEvent::UploadProgress {
            upload_id: "uuid-1".to_string(),
            status: UploadStatus::Uploading,
            parts_bitmask: "AAAA".to_string(),
            part_count: 4,
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"upload_progress""#));
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::UploadProgress { upload_id, .. } => {
                assert_eq!(upload_id, "uuid-1");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_sse_event_heartbeat_tag() {
        let event = SseEvent::Heartbeat { timestamp: 1700000000000 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"heartbeat""#));
    }

    #[test]
    fn test_sse_event_device_connected_tag() {
        let event = SseEvent::DeviceConnected {
            device_id: "dev-1".to_string(),
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"device_connected""#));
    }

    #[test]
    fn test_sse_event_upload_done_roundtrip() {
        let event = SseEvent::UploadDone {
            upload_id: "uuid-1".to_string(),
            file_id: 42,
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::UploadDone { file_id, .. } => assert_eq!(file_id, 42),
            _ => panic!("Wrong variant"),
        }
    }
}
```

#### `src/device.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_platform_serialization() {
        assert_eq!(serde_json::to_string(&DevicePlatform::Android).unwrap(), "\"android\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Ios).unwrap(), "\"ios\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Web).unwrap(), "\"web\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Desktop).unwrap(), "\"desktop\"");
    }

    #[test]
    fn test_device_info_roundtrip() {
        let di = DeviceInfo {
            device_id: "dev-1".to_string(),
            name: "My Phone".to_string(),
            platform: DevicePlatform::Android,
            sse_token: "token".to_string(),
            push_token: Some("push-token".to_string()),
            stall_timeout_seconds: 300,
        };
        let json = serde_json::to_string(&di).unwrap();
        let decoded: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, di.name);
        assert_eq!(decoded.platform, di.platform);
    }

    #[test]
    fn test_device_registration_roundtrip() {
        let dr = DeviceRegistration {
            name: "New Device".to_string(),
            platform: DevicePlatform::Desktop,
            push_token: None,
        };
        let json = serde_json::to_string(&dr).unwrap();
        let decoded: DeviceRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, dr.name);
    }
}
```

#### `src/share.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_record_roundtrip() {
        let sr = ShareRecord {
            file_id: 1,
            shared_with: "user-2".to_string(),
            collection_id: "col-1".to_string(),
            encrypted_collection_key: "eck".to_string(),
            created_at: 1700000000000,
            expires_at: Some(1700086400000),
        };
        let json = serde_json::to_string(&sr).unwrap();
        let decoded: ShareRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.shared_with, sr.shared_with);
    }

    #[test]
    fn test_share_request_roundtrip() {
        let req = ShareRequest {
            file_id: 1,
            shared_with: "user-2".to_string(),
            collection_id: "col-1".to_string(),
            encrypted_collection_key: "eck".to_string(),
            expires_at: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ShareRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.file_id, req.file_id);
    }
}
```

#### `src/user.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyAttributes;

    #[test]
    fn test_user_registration_roundtrip() {
        let ur = UserRegistration {
            email: "test@example.com".to_string(),
            verify_key_hash: "vkh".to_string(),
            encrypted_master_key: "emk".to_string(),
            key_nonce: "kn".to_string(),
            kek_salt: "salt".to_string(),
            mem_limit: 256 * 1024 * 1024,
            ops_limit: 4,
            public_key: "pk".to_string(),
            encrypted_secret_key: "esk".to_string(),
            secret_key_nonce: "skn".to_string(),
            encrypted_recovery_key: "erk".to_string(),
            recovery_key_nonce: "rkn".to_string(),
        };
        let json = serde_json::to_string(&ur).unwrap();
        let decoded: UserRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.email, ur.email);
    }

    #[test]
    fn test_login_params_roundtrip() {
        let lp = LoginParams {
            kek_salt: "salt".to_string(),
            mem_limit: 128 * 1024 * 1024,
            ops_limit: 3,
        };
        let json = serde_json::to_string(&lp).unwrap();
        let decoded: LoginParams = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mem_limit, lp.mem_limit);
    }

    #[test]
    fn test_login_response_roundtrip() {
        let lr = LoginResponse {
            session_token: "token".to_string(),
            key_attributes: KeyAttributes {
                encrypted_master_key: "emk".to_string(),
                key_nonce: "kn".to_string(),
                kek_salt: "salt".to_string(),
                mem_limit: 256 * 1024 * 1024,
                ops_limit: 4,
            },
        };
        let json = serde_json::to_string(&lr).unwrap();
        let decoded: LoginResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.session_token, lr.session_token);
    }

    #[test]
    fn test_session_info_roundtrip() {
        let si = SessionInfo {
            user_id: "user-1".to_string(),
            expires_at: 1700086400000,
        };
        let json = serde_json::to_string(&si).unwrap();
        let decoded: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.user_id, si.user_id);
    }
}
```

#### `src/error.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serialization() {
        assert_eq!(serde_json::to_string(&ErrorCode::Unauthorized).unwrap(), "\"unauthorized\"");
        assert_eq!(serde_json::to_string(&ErrorCode::NotFound).unwrap(), "\"not_found\"");
        assert_eq!(serde_json::to_string(&ErrorCode::UploadAlreadyExists).unwrap(), "\"upload_already_exists\"");
        assert_eq!(serde_json::to_string(&ErrorCode::InvalidStateTransition).unwrap(), "\"invalid_state_transition\"");
        assert_eq!(serde_json::to_string(&ErrorCode::RateLimited).unwrap(), "\"rate_limited\"");
    }

    #[test]
    fn test_api_error_roundtrip() {
        let err = ApiError {
            code: ErrorCode::ValidationError,
            message: "Invalid email".to_string(),
            details: Some(serde_json::json!({"field": "email"})),
        };
        let json = serde_json::to_string(&err).unwrap();
        let decoded: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.code, err.code);
        assert_eq!(decoded.message, err.message);
    }

    #[test]
    fn test_error_response_roundtrip() {
        let resp = ErrorResponse {
            error: ApiError {
                code: ErrorCode::InternalError,
                message: "Something went wrong".to_string(),
                details: None,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.error.code, resp.error.code);
    }
}
```

## Verification Steps
- [ ] `cargo check -p types` succeeds
- [ ] `cargo test -p types` passes all round-trip tests
- [ ] All structs derive `Serialize` and `Deserialize`
- [ ] No I/O, no HTTP, no DB, no crypto dependencies in `Cargo.toml`
- [ ] `cargo tree -p types` shows only `serde`, `serde_json`, and `zeroize` as external deps

## Notes
- This crate has ZERO logic — only type definitions with derives.
- All string fields that represent base64-encoded bytes are `String` type (not `Vec<u8>`) because they travel over JSON APIs.
- The `zeroize` crate dependency is needed for `Key256` — add it to `Cargo.toml`.
- `Key256` should NOT implement `Default` or `PartialEq` to prevent accidental comparison of key material.
- Use `#[serde(transparent)]` on newtype wrappers (`Key256`, `Nonce24`, `Header24`, `Salt16`) so they serialize as raw arrays rather than nested objects.
- The `#[serde(default)]` attribute on `FileMetadata::tags` ensures empty arrays instead of null when deserializing from older API versions.
- `EncryptedFileRecord::cipher` defaults to `"xchacha20-poly1305"` via `#[serde(default = "default_cipher")]` for backward compatibility.
- All timestamp fields are `i64` representing Unix milliseconds — consistent across the entire codebase.
- The `SseEvent` enum uses internally tagged serialization (`#[serde(tag = "type")]`) so the discriminator appears alongside variant fields in the JSON object.
