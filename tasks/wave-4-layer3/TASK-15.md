# Task 15: Implement `crates/client-lib` — Layer 3 Tauri Command Layer

## Wave
4 (Layer 3 — Platform Bindings)

## Dependencies
- All Wave 0–3 tasks must be complete (types, common, crypto, image, metadata, thumbnail, local-db, sync, zoo-client)

## Can Run In Parallel With
- Task 16 (zoo-wasm) — no dependencies between them

## Design References
- design.md §10.1: Client Lib Module Structure
- design.md §10.2: Tauri Commands
- design.md §10.3: AppState
- design.md §11.1: Desktop (Tauri)
- design.md §12.3: Memory Protection

## Requirements
1.1–1.12, 2.1–2.10, 4.1–4.2, 5.1–5.3, 8.8, 10.1–10.8, 11.2–11.4, 12.1–12.2, 20.6, 21.1–21.4, 22.2–22.7, 25.5, 26.1–26.5

## Objective
Tauri command layer that wires sync + local-db + thumbnail + zoo-client. Thin wrappers — no business logic.

## CRITICAL CONSTRAINT
- **NO `axum`, `sqlx` (postgres), `aws-sdk-s3` dependencies** — enforced at compile time

## Cargo.toml
```toml
[package]
name = "client-lib"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[features]
default = ["desktop"]
desktop = ["tauri", "tauri-specta"]

[dependencies]
types = { workspace = true }
sync = { workspace = true }
local-db = { workspace = true }
thumbnail = { workspace = true }
zoo-client = { workspace = true }
common = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
tauri = { workspace = true, optional = true }
tauri-specta = { workspace = true, optional = true }
specta = { workspace = true, optional = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod state;
pub mod commands;

pub use state::AppState;
```

### `src/state.rs` — AppState
```rust
pub struct AppState {
    pub db: Arc<LocalDb>,
    pub master_key: Arc<RwLock<Option<Key256>>>,  // ZeroizeOnDrop, None when logged out
    pub session_token: Arc<RwLock<Option<String>>>,
    pub device_info: Arc<RwLock<Option<DeviceInfo>>>,
    pub sync_state: Arc<RwLock<SyncState>>,
    pub thumbnail_cache: Arc<ThumbnailCache>,
    pub zoo_client: Arc<ZooClient>,
    pub config: Arc<AppConfig>,
}
```
- `AppState::init(config: AppConfig) -> Result<Self, AppError>`:
  - `LocalDb::open(&config.db_path)`
  - `ThumbnailCache::new(500, config.cache_dir.join("thumbnails"), 2 * 1024 * 1024 * 1024)`
  - `ZooClient::new(config.server_url.clone())`
  - All fields wrapped in `Arc<RwLock<...>>`

### `src/commands/mod.rs`
Register all command modules:
```rust
pub mod auth;
pub mod collections;
pub mod files;
pub mod sync;
pub mod upload;
pub mod thumbnails;
pub mod device;
pub mod search;

#[cfg(feature = "desktop")]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        auth::login, auth::logout, auth::register, auth::get_auth_params,
        collections::list_collections, collections::create_collection, collections::archive_collection,
        files::list_files, files::get_file, files::archive_file, files::download_file,
        sync::trigger_sync, sync::get_sync_status,
        upload::upload_file, upload::cancel_upload, upload::list_pending_uploads,
        thumbnails::get_thumbnail, thumbnails::evict_thumbnail,
        device::register_device, device::get_device_info,
        search::search_files, search::search_by_date, search::search_by_location,
    ])
}
```

### `src/commands/auth.rs`
- `get_auth_params(email, state)` → POST /api/auth/params via zoo_client
- `login(email, password, state)`:
  1. POST /api/auth/params → get kek_salt, mem_limit, ops_limit
  2. `crypto::derive_kek(password, salt, profile)` → KEK
  3. `crypto::secretbox_decrypt(encrypted_master_key, key_nonce, KEK)` → MasterKey
  4. Store MasterKey in `state.master_key` (secret memory)
  5. Store session_token in `state.session_token`
- `logout(state)`: zeroize MasterKey (`*state.master_key.write() = None`), revoke session
- `register(email, password, state)`:
  1. Derive KEK, generate MasterKey, CollectionKey, X25519 keypair, RecoveryKey
  2. Encrypt all keys with KEK/MasterKey
  3. Derive VerificationKey → SHA-256 → verify_key_hash
  4. POST /api/auth/register

### `src/commands/collections.rs`
- `list_collections(state)` → `local_db::collections::list_collections`
- `create_collection(name, state)`:
  1. `crypto::generate_key()` → CollectionKey
  2. `crypto::secretbox_encrypt(CollectionKey, MasterKey)` → encrypted
  3. POST to Zoo (if online)
  4. `local_db::collections::upsert_collection`
- `archive_collection(id, state)` → `local_db::collections::archive_collection`

### `src/commands/files.rs`
- `list_files(collection_id, state)` → `local_db::files::list_files`
- `get_file(file_id, state)` → `local_db::files::get_file`
- `archive_file(file_id, state)` → `local_db::files::archive_files`
- `download_file(file_id, destination, state)`:
  1. Get FileRecord from local DB
  2. `zoo_client::download_file(file_id)` → encrypted bytes
  3. `crypto::stream_decrypt(file_decryption_header, encrypted, file_key)` → plaintext
  4. Write to destination path

### `src/commands/upload.rs`
- `upload_file(file_path, collection_id, state)`:
  1. Read file bytes
  2. `image::extract_exif(&bytes)` → EXIF data
  3. Build FileMetadata from EXIF
  4. `thumbnail::generate_thumbnail(&bytes, mime_type, file_key)` → encrypted thumbnail
  5. `crypto::stream_encrypt(&file_bytes, file_key)` → encrypted file
  6. `metadata::encrypt_metadata(&metadata, file_key)` → encrypted metadata
  7. `zoo_client::upload_file(encrypted_bytes, metadata)` → orchestrator
- `cancel_upload(upload_id, state)` → `zoo_client::cancel_upload`
- `list_pending_uploads(state)` → `zoo_client::pending_uploads`

### `src/commands/sync.rs`
- `trigger_sync(state)`: check connectivity, call `sync::sync_all`
- `get_sync_status(state)`: return last sync time, in-progress flag

### `src/commands/thumbnails.rs`
- `get_thumbnail(file_id, state)`: delegate to `ThumbnailCache::get`
- `evict_thumbnail(file_id, state)`: delegate to `ThumbnailCache::evict`

### `src/commands/device.rs`
- `register_device(name, platform, state)`: POST /api/devices, store in AppState
- `get_device_info(state)`: return device info from AppState

### `src/commands/search.rs`
- `search_files(query, state)`: FTS5 query via `local_db::search::search_text`
- `search_by_date(start_ms, end_ms, state)`: `local_db::search::search_by_date`
- `search_by_location(lat_min, lat_max, lon_min, lon_max, state)`: `local_db::search::search_by_location`

### `src/commands/error.rs` (or use CommonError)
```rust
#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("not logged in")]
    NotLoggedIn,
    #[error("sync error: {0}")]
    SyncError(#[from] sync::error::SyncError),
    #[error("database error: {0}")]
    DbError(#[from] local_db::error::LocalDbError),
    #[error("zoo error: {0}")]
    ZooError(#[from] zoo_client::error::ZooError),
    #[error("crypto error: {0}")]
    CryptoError(#[from] crypto::error::CryptoError),
    #[error("thumbnail error: {0}")]
    ThumbnailError(#[from] thumbnail::ThumbnailError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Tests (Task 15.12 — marked with *)
Integration tests using mock dependencies:
- `login` command decrypts MasterKey and stores in AppState
- `upload_file` command encrypts file and calls zoo-client orchestrator
- `get_thumbnail` returns from memory cache on hit
- `search_files` executes FTS5 query
- `trigger_sync` calls `sync_all`
- MasterKey is zeroized and absent from AppState after logout

## Verification Steps
- [ ] `cargo check -p client-lib` succeeds
- [ ] `cargo test -p client-lib` passes
- [ ] NO axum/sqlx-postgres/aws-sdk-s3 in `cargo tree -p client-lib`
- [ ] All commands have `#[tauri::command]` and `#[specta::specta]` attributes
- [ ] MasterKey is `None` after logout
- [ ] `AppState::init` creates all sub-components

## Notes
- This crate is a thin orchestration layer — all business logic is in the lower crates.
- The `desktop` feature flag gates the `tauri` dependency. Without it, the crate can be tested without Tauri.
- `tauri_specta` generates TypeScript bindings from Rust command definitions.
- All commands take `tauri::State<'_, AppState>` as a parameter for shared state access.
- The `Key256` type implements `ZeroizeOnDrop` — when `master_key` is set to `None`, the old key is zeroized.
