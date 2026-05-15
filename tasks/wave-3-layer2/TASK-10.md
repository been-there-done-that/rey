# Task 10: Implement `crates/local-db` — Layer 2 Encrypted SQLite Database

## Wave
3 (Layer 2 — Application Logic)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 3 (common) must be complete

## Can Run In Parallel With
- Task 11 (zoo server), Task 12 (zoo-client) — no dependencies between them
- Task 13 (sync) depends on this task + all of Wave 2

## Design References
- design.md §7.1: Local DB Module Structure
- design.md §7.2: SQLite Schema (collections, files, sync_state, FTS5)
- design.md §7.3: Encryption at Rest (SQLCipher + platform keychain)
- design.md §7.4: Search Queries (FTS5, date range, geographic)
- SPEC.md §2.5: Local SQLite Schema
- SPEC.md §2.6: First Sync vs Incremental
- SPEC.md §5.2: Local Database Security

## Requirements
9.1–9.7, 21.1–21.6, 25.4

## Objective
Manage the local SQLite database of decrypted metadata. SQLCipher encryption at rest. FTS5 full-text search. CRUD operations for collections, files, sync cursors.

## Cargo.toml
```toml
[package]
name = "local-db"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[features]
default = ["sqlcipher"]
sqlcipher = ["rusqlite/sqlcipher"]

[dependencies]
types = { workspace = true }
common = { workspace = true }
rusqlite = { workspace = true }
keyring = { workspace = true }
thiserror = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod connection;
pub mod collections;
pub mod files;
pub mod sync_state;
pub mod search;

pub use connection::LocalDb;
```

### SQL Migrations

Create `migrations/001_initial.sql`:
```sql
CREATE TABLE IF NOT EXISTS collections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    encrypted_key   TEXT NOT NULL,
    key_nonce       TEXT NOT NULL,
    updation_time   INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    archived_at     INTEGER
);

CREATE TABLE IF NOT EXISTS files (
    id                      INTEGER PRIMARY KEY,
    collection_id           TEXT NOT NULL REFERENCES collections(id),
    cipher                  TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    title                   TEXT,
    description             TEXT,
    latitude                REAL,
    longitude               REAL,
    taken_at                INTEGER,
    file_size               INTEGER NOT NULL,
    mime_type               TEXT NOT NULL,
    content_hash            TEXT NOT NULL,
    encrypted_key           TEXT NOT NULL,
    key_nonce               TEXT NOT NULL,
    file_decryption_header  TEXT NOT NULL,
    thumb_decryption_header TEXT,
    object_key              TEXT NOT NULL,
    thumbnail_path          TEXT,
    updation_time           INTEGER NOT NULL,
    created_at              INTEGER NOT NULL,
    archived_at             INTEGER
);

CREATE INDEX IF NOT EXISTS idx_files_collection ON files(collection_id);
CREATE INDEX IF NOT EXISTS idx_files_taken_at ON files(taken_at);
CREATE INDEX IF NOT EXISTS idx_files_archived ON files(archived_at) WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_files_latitude ON files(latitude) WHERE latitude IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_files_longitude ON files(longitude) WHERE longitude IS NOT NULL;

CREATE TABLE IF NOT EXISTS sync_state (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);
```

Create `migrations/002_fts5.sql`:
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    title,
    description,
    content='files',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 1'
);

CREATE TRIGGER IF NOT EXISTS files_fts_insert AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_update AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_delete AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
END;
```

### `src/connection.rs`
Implement `LocalDb`:
- `open(db_path: &Path) -> Result<LocalDb, LocalDbError>`:
  1. Retrieve or generate 32-byte DB encryption key from platform keychain (`keyring` crate, service `"rey"`, username `"local_db_key"`)
  2. Open SQLite with SQLCipher: `PRAGMA key = 'hex_encoded_key'`
  3. Verify key works: `PRAGMA user_version` — if fails, return `LocalDbError::InvalidKey`
  4. Run pending migrations using embedded SQL files
  5. Return `LocalDb { conn }`
- `close(self)` — explicit close (optional, Drop handles it)
- `LocalDbError` enum: `KeychainUnavailable`, `InvalidKey`, `MigrationFailed(String)`, `QueryError(rusqlite::Error)`, `Io(std::io::Error)`

### `src/collections.rs`
Implement:
- `upsert_collection(conn: &Connection, collection: &Collection) -> Result<(), LocalDbError>`
- `list_collections(conn: &Connection) -> Result<Vec<Collection>, LocalDbError>` — WHERE archived_at IS NULL
- `get_collection_key(conn: &Connection, id: &str) -> Result<Option<EncryptedKey>, LocalDbError>`
- `archive_collection(conn: &Connection, id: &str) -> Result<(), LocalDbError>` — set archived_at = now_ms()

### `src/files.rs`
Implement:
- `upsert_files(conn: &Connection, files: &[FileRecord]) -> Result<(), LocalDbError>` — batch insert with ON CONFLICT REPLACE
- `archive_files(conn: &Connection, ids: &[i64]) -> Result<(), LocalDbError>` — set archived_at for each
- `list_files(conn: &Connection, collection_id: &str) -> Result<Vec<FileRecord>, LocalDbError>` — WHERE collection_id = ? AND archived_at IS NULL ORDER BY taken_at DESC
- `get_file(conn: &Connection, id: i64) -> Result<Option<FileRecord>, LocalDbError>`
- `list_files_without_thumbnail(conn: &Connection) -> Result<Vec<FileRecord>, LocalDbError>` — WHERE thumbnail_path IS NULL AND archived_at IS NULL

### `src/sync_state.rs`
Implement:
- `read_cursor(conn: &Connection, key: &str) -> Result<Option<i64>, LocalDbError>`
- `write_cursor(conn: &Connection, key: &str, value: i64) -> Result<(), LocalDbError>`
- Keys: `"collections_since"`, `"collection:{id}_since"`, `"trash_since"`

### `src/search.rs`
Implement:
- `search_text(conn: &Connection, query: &str, limit: usize) -> Result<Vec<FileRecord>, LocalDbError>`:
  ```sql
  SELECT f.* FROM files f
  JOIN files_fts fts ON fts.rowid = f.id
  WHERE files_fts MATCH ?
    AND f.archived_at IS NULL
  ORDER BY f.taken_at DESC
  LIMIT ?
  ```
- `search_by_date(conn: &Connection, start_ms: i64, end_ms: i64, limit: usize) -> Result<Vec<FileRecord>, LocalDbError>`
- `search_by_location(conn: &Connection, lat_min: f64, lat_max: f64, lon_min: f64, lon_max: f64, limit: usize) -> Result<Vec<FileRecord>, LocalDbError>`
- `rebuild_fts_index(conn: &Connection) -> Result<(), LocalDbError>`: `INSERT INTO files_fts(files_fts) VALUES('rebuild')`

## Tests (Task 10.9 — marked with *)
Integration tests using `tempfile::TempDir`:
- Open DB with SQLCipher key (use a test key, not real keychain)
- Migrations run in order (001 then 002)
- Collections CRUD: upsert, list, get_key, archive
- Files upsert and archive
- sync_state read/write cursors
- FTS5 text search returns correct results
- FTS5 index rebuild
- Date range query returns correct results
- Geographic bounding box query returns correct results
- Keychain unavailable returns `LocalDbError::KeychainUnavailable` without opening unencrypted DB

## Verification Steps
- [ ] `cargo check -p local-db` succeeds
- [ ] `cargo test -p local-db` passes (requires SQLCipher bundled)
- [ ] DB is encrypted — opening with wrong key fails
- [ ] FTS5 triggers keep index in sync with files table
- [ ] All CRUD operations work correctly
- [ ] Search queries return correct results with test data

## Notes
- SQLCipher is enabled via `rusqlite` with the `sqlcipher` feature and `bundled` feature (compiles SQLite + SQLCipher from source).
- The keychain integration uses the `keyring` crate which abstracts over macOS Keychain, Windows DPAPI, and Linux Secret Service.
- For tests, bypass the keychain and use a hardcoded test key.
- FTS5 `unicode61 remove_diacritics 1` tokenizer handles international text and removes diacritical marks for better matching.
- The `files_fts` virtual table is a content table — it doesn't store data itself, it indexes the `files` table.
