# Task 13: Implement `crates/sync` — Layer 2 Incremental Sync Engine

## Wave
3 (Layer 2 — Application Logic)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 3 (common) must be complete
- Task 5 (crypto) must be complete
- Task 7 (metadata) must be complete
- Task 8 (thumbnail) must be complete
- Task 10 (local-db) must be complete
- Task 12 (zoo-client) must be complete

## Can Run In Parallel With
Nothing in this wave — sync depends on many prior tasks

## Design References
- design.md §6.1: Sync Module Structure
- design.md §6.2: Sync Flow (sync_all algorithm)
- design.md §6.3: Version-Consistent Pagination
- design.md §6.4: Decryption Pipeline
- SPEC.md §2.2: API Endpoints
- SPEC.md §2.4: Client Sync Flow
- SPEC.md §2.6: First Sync vs Incremental
- SPEC.md §2.7: Offline Support

## Requirements
7.1–7.7, 8.1–8.8, 11.4, 25.4

## Objective
Pull diffs from Zoo, decrypt, write to local DB. Cursor tracking. Batch decryption. Thumbnail queueing.

## Cargo.toml
```toml
[package]
name = "sync"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
crypto = { workspace = true }
metadata = { workspace = true }
thumbnail = { workspace = true }
local-db = { workspace = true }
common = { workspace = true }
zoo-client = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod pull;
pub mod diff;
pub mod decrypt;
pub mod thumbnails;
pub mod cursor;
pub mod error;

pub use pull::sync_all;
pub use pull::SyncEngine;
```

### `src/cursor.rs`
- `read(db: &LocalDb, key: &str) -> Result<Option<i64>, SyncError>` — delegates to `local_db::sync_state::read_cursor`
- `write(db: &LocalDb, key: &str, value: i64) -> Result<(), SyncError>` — delegates to `local_db::sync_state::write_cursor`
- Keys: `"collections_since"`, `"collection:{id}_since"`, `"trash_since"`

### `src/decrypt.rs`
`batch_decrypt_files(records: &[EncryptedFileRecord], collection_key: &Key256) -> Result<Vec<FileRecord>, SyncError>`:
- For each record:
  1. `crypto::secretbox_decrypt` FileKey with CollectionKey
  2. `crypto::stream_decrypt` metadata with FileKey
  3. `serde_json::from_slice` → FileMetadata
  4. Build FileRecord
- On decryption failure: log error, skip record, continue (non-fatal per Req 7.7)

### `src/diff.rs`
- `fetch_collection_page(client: &ZooClient, since: i64) -> Result<SyncCollectionResponse, SyncError>`
- `fetch_file_page(client: &ZooClient, collection_id: &str, since: i64, limit: usize) -> Result<SyncFilesResponse, SyncError>`
- `fetch_trash_page(client: &ZooClient, since: i64) -> Result<SyncTrashResponse, SyncError>`

### `src/pull.rs`
`SyncEngine` struct with `zoo_client`, `local_db`, `thumbnail_cache`, `master_key`.

`sync_all(engine: &SyncEngine) -> Result<(), SyncError>`:
1. Sync collections: loop with `has_more`, persist cursor after each page
   - Decrypt collection name and key with MasterKey
   - Upsert into local DB
2. Sync files per collection: loop with `has_more`, persist cursor
   - Batch decrypt files
   - Upsert into local DB
   - Handle deleted_file_ids (archive)
3. Sync trash: loop with `has_more`, persist cursor
   - Archive deleted files
4. Queue thumbnail downloads for new files

**Offline behavior**: If network unavailable, return immediately without network calls. All reads go to local DB.

### `src/thumbnails.rs`
`queue_new_files(db: &LocalDb, cache: &ThumbnailCache) -> Result<(), SyncError>`:
- List files without `thumbnail_path`
- Enqueue thumbnail download tasks via `ThumbnailCache::get`

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum SyncError {
    #[error("network error: {0}")]
    NetworkError(#[from] zoo_client::error::ZooError),
    #[error("decryption failed for file {file_id}: {source}")]
    DecryptionFailed { file_id: i64, source: crypto::error::CryptoError },
    #[error("database error: {0}")]
    DbError(#[from] local_db::error::LocalDbError),
    #[error("cursor error: {0}")]
    CursorError(String),
    #[error("offline mode: no network available")]
    Offline,
}
```

## Tests (Tasks 13.7–13.8 — marked with *)

### Integration Tests (13.7)
Using `wiremock`:
- `sync_all()` fetches collections then files then trash in order
- Cursor persisted after each page
- `has_more=true` triggers next page fetch
- Decryption failure skips record and continues
- Version-consistent pagination discards incomplete last group
- Offline mode serves from local DB without network calls

### Property Test (13.8)
- `∀ page_size, records with varying updation_times: sync_all() with paginated responses always advances cursor monotonically and never processes the same record twice`

## Verification Steps
- [ ] `cargo check -p sync` succeeds
- [ ] `cargo test -p sync` passes
- [ ] Sync flow: collections → files → trash in correct order
- [ ] Cursor persistence after each page
- [ ] Decryption failure skips record, continues sync
- [ ] Offline mode returns without network calls
- [ ] Thumbnail queueing works for new files

## Notes
- The sync engine is the bridge between the Zoo HTTP API and the local SQLite database.
- Decryption failures are non-fatal — a corrupted record is skipped but sync continues.
- The cursor is persisted after EACH page, not just at the end, to allow resuming after partial sync.
- Offline mode: the caller (client-lib) checks connectivity before calling sync_all. If offline, sync_all returns immediately.
