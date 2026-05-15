# Implementation Plan: Rey — End-to-End Encrypted Photo Storage

## Overview

This plan builds the Rey workspace in strict layer order, matching the dependency DAG in the design document. Each layer is fully compilable before the next begins. Property-based tests (using `proptest`) are placed immediately after the code they validate to catch regressions early. All tasks are Rust unless explicitly noted as TypeScript/Next.js.

---

## Tasks

- [ ] 1. Scaffold the Cargo workspace and shared tooling
  - Create `Cargo.toml` virtual manifest (no `[package]` section) with `[workspace]` listing all crates under `crates/` and the two app crates under `apps/`
  - Add `[workspace.dependencies]` table pinning all shared third-party crates (serde, tokio, axum, sqlx, reqwest, argon2, etc.) so individual `Cargo.toml` files use `{ workspace = true }`
  - Create `pnpm-workspace.yaml`, root `package.json`, and `turbo.json` for the JS monorepo
  - Create `Makefile` with top-level targets: `build`, `test`, `lint`, `fmt`, `gen-openapi`, `gen-bindings`
  - Create `.github/workflows/ci.yml` running `cargo test --workspace --all-features`, `cargo clippy -- -D warnings`, `cargo fmt --check`, and `pnpm turbo test`
  - Create empty `crates/` directory stubs for all twelve crates so `cargo check --workspace` resolves without errors
  - _Requirements: 25.1, 25.7_

- [ ] 2. Implement `crates/types` — Layer 0 shared data types
  - [ ] 2.1 Create `crates/types/Cargo.toml` with only `serde` and `serde_json` as dependencies (no I/O, no HTTP, no DB, no crypto)
    - _Requirements: 25.2_
  - [ ] 2.2 Implement `src/crypto.rs`: `Key256`, `Nonce24`, `Header24`, `Salt16`, `EncryptedKey`, `KeyAttributes`, `Argon2Profile` with `Serialize`/`Deserialize` derives
    - _Requirements: 1.1–1.8, 3.5–3.6_
  - [ ] 2.3 Implement `src/file.rs`: `FileMetadata`, `EncryptedFileRecord` with all fields from design §4.2
    - _Requirements: 5.2, 8.2, 9.2_
  - [ ] 2.4 Implement `src/collection.rs`: `Collection`, `EncryptedCollection`
    - _Requirements: 4.1, 7.2_
  - [ ] 2.5 Implement `src/sync.rs`: `SyncCollectionResponse`, `SyncFilesResponse`, `SyncTrashResponse`, `SyncCursor`, `DeletedFileRef`
    - _Requirements: 7.2, 8.2, 8.7_
  - [ ] 2.6 Implement `src/upload.rs`: `UploadStatus` enum (all 8 variants, `serde(rename_all = "snake_case")`), `UploadState`, `PartRecord`, `UploadSummary`
    - _Requirements: 13.1, 13.5, 28.4_
  - [ ] 2.7 Implement `src/sse.rs`: `SseEvent` enum with all variants from design §4.2 (`UploadProgress`, `UploadCompleted`, `UploadDone`, `UploadStalled`, `UploadFailed`, `UploadPending`, `DeviceConnected`, `DeviceDisconnected`, `Heartbeat`)
    - _Requirements: 19.4, 19.5_
  - [ ] 2.8 Implement `src/device.rs`, `src/share.rs`, `src/user.rs`, `src/error.rs` with all protocol types
    - _Requirements: 12.1, 6.4, 2.5_
  - [ ]* 2.9 Write unit tests for `types` crate: serde round-trips for all structs; `UploadStatus` serializes to snake_case strings; `SseEvent` tag field serialization; `Argon2Profile` mem/ops values match spec
    - _Requirements: 25.2_

- [ ] 3. Implement `crates/common` — Layer 0 shared utilities
  - [ ] 3.1 Create `crates/common/Cargo.toml` with `serde`, `tracing`, `thiserror` dependencies
    - _Requirements: 25.1_
  - [ ] 3.2 Implement config parsing helpers (env-var driven), tracing initialisation, and shared error formatting utilities
    - _Requirements: 25.1_
  - [ ]* 3.3 Write unit tests for config parsing from env vars and error formatting
    - _Requirements: 25.1_

- [ ] 4. Checkpoint — Layer 0 complete
  - Ensure `cargo test -p types -p common` passes. Ask the user if questions arise.

- [ ] 5. Implement `crates/crypto` — Layer 1 cryptographic primitives
  - [ ] 5.1 Create `crates/crypto/Cargo.toml` with `types`, `aead`, `chacha20poly1305`, `xsalsa20poly1305`, `x25519-dalek`, `argon2`, `blake2b_simd`, `rand_core`, `zeroize` dependencies; mark crate `#![no_std]` compatible
    - _Requirements: 25.3_
  - [ ] 5.2 Implement `src/error.rs`: `CryptoError` enum with `MacMismatch`, `UnsupportedCipher`, `AllocationFailed`, `InvalidKey`, `InvalidNonce` variants
    - _Requirements: 5.6, 5.7_
  - [ ] 5.3 Implement `src/aead/secretbox.rs`: `secretbox_encrypt(plaintext, key) -> (Nonce24, Vec<u8>)` and `secretbox_decrypt(nonce, ciphertext, key) -> Result<Vec<u8>, CryptoError>` using XSalsa20-Poly1305; wire format: `nonce(24) || MAC(16) || ciphertext`; never return partial plaintext on MAC failure
    - _Requirements: 1.3, 4.2, 4.4, 5.5, 5.6, 5.7_
  - [ ] 5.4 Implement `src/aead/stream.rs`: `stream_encrypt(plaintext, key) -> (Header24, Vec<u8>)` and `stream_decrypt(header, ciphertext, key) -> Result<Vec<u8>, CryptoError>` using XChaCha20-Poly1305 SecretStream; wire format: `header(24) || ciphertext`; verify MAC before returning any bytes
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.6, 5.7_
  - [ ] 5.5 Implement `src/kdf/argon.rs`: `derive_kek(password, salt, profile) -> Result<Key256, CryptoError>` with Argon2id v1.3 and the adaptive fallback loop (halve mem, double ops, floor at 32 MiB)
    - _Requirements: 1.1, 3.1–3.4_
  - [ ] 5.6 Implement `src/kdf/blake2b.rs`: `derive_verification_key(kek) -> Key256` (context `"verification"`, subkey_id 2) and `derive_subkey(master, context, id) -> Key256`
    - _Requirements: 1.4, 2.3_
  - [ ] 5.7 Implement `src/key/generate.rs`: `generate_key() -> Key256` using `OsRng`; `src/key/encrypt.rs` and `src/key/decrypt.rs` wrapping secretbox for key-wrapping operations
    - _Requirements: 1.2, 4.1, 4.3_
  - [ ] 5.8 Implement `src/seal/keypair.rs`: `generate_keypair() -> (SecretKey, PublicKey)` via `OsRng`; `src/seal/box_.rs`: `seal(plaintext, recipient_pk) -> Vec<u8>` and `open(ciphertext, recipient_sk) -> Result<Vec<u8>, CryptoError>` using X25519 ephemeral keypair + XSalsa20-Poly1305; wire format: `ephemeral_pk(32) || MAC(16) || ciphertext`
    - _Requirements: 1.6, 1.7, 6.1, 6.2, 6.3_
  - [ ] 5.9 Implement `src/util.rs`: `constant_time_eq`, zeroize helpers, base64/hex encoding utilities
    - _Requirements: 2.4_
  - [ ]* 5.10 Write property test: `∀ key ∈ Key256, plaintext ∈ Vec<u8>: stream_decrypt(stream_encrypt(plaintext, key)) == Ok(plaintext)`
    - **Property 1: SecretStream round-trip**
    - **Validates: Requirements 5.8**
  - [ ]* 5.11 Write property test: `∀ key ∈ Key256, plaintext ∈ Vec<u8>: secretbox_decrypt(secretbox_encrypt(plaintext, key)) == Ok(plaintext)`
    - **Property 2: SecretBox round-trip**
    - **Validates: Requirements 1.3, 4.2, 4.4**
  - [ ]* 5.12 Write property test: `∀ key ∈ Key256, ciphertext ∈ Vec<u8> (modified by flipping one byte): stream_decrypt(ciphertext, key) == Err(MacMismatch)` — verify no partial plaintext is returned
    - **Property 3: MAC mismatch returns error, no partial plaintext**
    - **Validates: Requirements 5.6, 5.7**
  - [ ]* 5.13 Write property test: `∀ recipient_keypair, plaintext: seal_open(seal(plaintext, pk), sk) == Ok(plaintext)`
    - **Property 4: SealedBox round-trip**
    - **Validates: Requirements 6.1, 6.2, 6.3**
  - [ ]* 5.14 Write unit tests: Argon2id KAT against libsodium reference vectors; adaptive fallback halves memory and doubles ops; fallback stops at 32 MiB floor; BLAKE2b-KDF VerificationKey derivation matches reference
    - _Requirements: 1.1, 3.1–3.6_

- [ ] 6. Implement `crates/image` — Layer 1 image decoding and EXIF
  - [ ] 6.1 Create `crates/image/Cargo.toml` with `types`, `common`, `image` (image-rs), `kamadak-exif` dependencies; no `crypto` dependency
    - _Requirements: 25.3, 26_
  - [ ] 6.2 Implement `src/decode.rs`: decode JPEG, PNG, WebP, HEIC source bytes into an in-memory image; return `ImageError::UnsupportedFormat` for unknown types
    - _Requirements: 26.1_
  - [ ] 6.3 Implement `src/exif.rs`: extract GPS coordinates (latitude, longitude), capture datetime (`taken_at`), device make, device model, and orientation tag from EXIF; return partial result with `None` fields when EXIF is absent or malformed (no error)
    - _Requirements: 26.1, 26.4_
  - [ ] 6.4 Implement `src/orientation.rs`: apply EXIF orientation correction (all 8 orientations) to a decoded image before encoding
    - _Requirements: 26.2, 10.5_
  - [ ] 6.5 Implement `src/resize.rs`: `max_dimension(image, max_px) -> Image` preserving aspect ratio
    - _Requirements: 10.1_
  - [ ] 6.6 Implement `src/encode.rs`: `jpeg(image, quality) -> Vec<u8>` JPEG encoder
    - _Requirements: 10.2_
  - [ ]* 6.7 Write unit tests with fixture images: JPEG/PNG/WebP decode succeeds; EXIF GPS extraction from fixture; EXIF orientation correction for all 8 orientations; resize to max 720px preserves aspect ratio; missing EXIF returns partial result without error
    - _Requirements: 26.1–26.4_

- [ ] 7. Implement `crates/metadata` — Layer 1 metadata encryption
  - [ ] 7.1 Create `crates/metadata/Cargo.toml` with `types`, `crypto`, `serde_json` dependencies
    - _Requirements: 25.3_
  - [ ] 7.2 Implement `src/lib.rs`: `encrypt_metadata(metadata: &FileMetadata, file_key: &Key256) -> (Header24, Vec<u8>)` — serialize `FileMetadata` to JSON, then `stream_encrypt`; `decrypt_metadata(header, ciphertext, file_key) -> Result<FileMetadata, MetadataError>` — `stream_decrypt` then `serde_json::from_slice`
    - _Requirements: 5.2, 26.3, 26.5_
  - [ ]* 7.3 Write property test: `∀ key ∈ Key256, metadata ∈ FileMetadata: decrypt_metadata(encrypt_metadata(metadata, key)) == Ok(metadata)`
    - **Property 5: Metadata round-trip**
    - **Validates: Requirements 5.2**
  - [ ]* 7.4 Write unit tests: missing optional fields serialize as `null`; JSON round-trip; GPS coordinates preserved with full f64 precision
    - _Requirements: 5.2, 26.3_

- [ ] 8. Implement `crates/thumbnail` — Layer 1 thumbnail pipeline
  - [ ] 8.1 Create `crates/thumbnail/Cargo.toml` with `types`, `crypto`, `image` (internal crate), `metadata`, `lru`, `dashmap`, `tokio` dependencies
    - _Requirements: 25.3_
  - [ ] 8.2 Implement `src/generate.rs`: `generate_thumbnail(source, mime_type, file_key) -> Result<(Header24, Vec<u8>), ThumbnailError>` — decode → apply EXIF orientation → resize to max 720px → encode JPEG at quality 85 → iterative quality reduction if > 100 KB → `stream_encrypt` with FileKey; dispatch on `mime_type` for image vs. video; return `ThumbnailError::UnsupportedFormat` on failure without panicking
    - _Requirements: 10.1–10.6, 10.8_
  - [ ] 8.3 Implement `src/encrypt.rs` and `src/decrypt.rs`: thin wrappers over `crypto::stream_encrypt` / `stream_decrypt` for thumbnail bytes
    - _Requirements: 10.6, 10.7_
  - [ ] 8.4 Implement `src/cache/memory.rs`: `MemoryCache` wrapping `lru::LruCache` with capacity 500
    - _Requirements: 11.1, 11.2_
  - [ ] 8.5 Implement `src/cache/disk.rs`: `DiskCache` storing files at `{app_cache_dir}/thumbnails/{file_id}`; maintain a metadata index (file_id → path, size, last_accessed) in a small SQLite file; evict LRU entries when total size exceeds 2 GB
    - _Requirements: 11.1, 11.3, 11.7_
  - [ ] 8.6 Implement `src/inflight.rs`: `InflightMap` using `DashMap<FileId, Arc<tokio::sync::Notify>>` for in-flight request deduplication
    - _Requirements: 11.5_
  - [ ] 8.7 Implement `src/cache/mod.rs`: `ThumbnailCache::get()` — check L1 memory → check L2 disk → deduplicate in-flight → download + decrypt + populate both levels; `ThumbnailCache::evict()` — remove from both levels
    - _Requirements: 11.2–11.6, 11.8_
  - [ ] 8.8 Implement `src/download.rs`: fetch encrypted thumbnail from Zoo via `GET /api/files/{file_id}/thumbnail`, decrypt with FileKey, write to L2 disk cache, insert into L1 memory cache
    - _Requirements: 11.4_
  - [ ]* 8.9 Write property test: `∀ key ∈ Key256, thumbnail_bytes ∈ Vec<u8>: decrypt_thumbnail(encrypt_thumbnail(thumbnail_bytes, key)) == Ok(thumbnail_bytes)`
    - **Property 6: Thumbnail round-trip**
    - **Validates: Requirements 10.7**
  - [ ]* 8.10 Write unit tests with fixture images: output ≤ 720px and ≤ 100 KB; iterative quality reduction triggers when output > 100 KB; EXIF orientation applied before encode; unsupported format returns error without panicking
    - _Requirements: 10.1–10.8_

- [ ] 9. Checkpoint — Layer 1 complete
  - Ensure `cargo test -p crypto -p image -p metadata -p thumbnail` passes. Ask the user if questions arise.

- [ ] 10. Implement `crates/local-db` — Layer 2 encrypted SQLite database
  - [ ] 10.1 Create `crates/local-db/Cargo.toml` with `types`, `common`, `rusqlite` (with `sqlcipher` feature), `rusqlite-migration`, `keyring` dependencies
    - _Requirements: 9.6, 25.4_
  - [ ] 10.2 Write SQL migration `001_initial.sql`: `collections` table (Req 9.1), `files` table (Req 9.2), `sync_state` table (Req 9.3), and all required indexes (Req 9.4)
    - _Requirements: 9.1–9.4_
  - [ ] 10.3 Write SQL migration `002_fts5.sql`: FTS5 virtual table `files_fts` with `content='files'`, `tokenize='unicode61 remove_diacritics 1'`; insert/update/delete triggers to keep FTS5 index in sync
    - _Requirements: 21.1_
  - [ ] 10.4 Implement `src/connection.rs`: `open(db_path) -> Result<LocalDb, LocalDbError>` — retrieve or generate DB encryption key from platform keychain (`keyring` crate, service `"rey"`, username `"local_db_key"`); open SQLite with SQLCipher `PRAGMA key`; verify key works; run pending migrations; return `LocalDbError::KeychainUnavailable` if keychain is unavailable (never open unencrypted)
    - _Requirements: 9.5, 9.6, 9.7_
  - [ ] 10.5 Implement `src/collections.rs`: `upsert_collection`, `list_collections`, `get_collection_key`, `archive_collection` CRUD operations
    - _Requirements: 9.1, 7.4_
  - [ ] 10.6 Implement `src/files.rs`: `upsert_files`, `archive_files`, `list_files`, `get_file` CRUD operations
    - _Requirements: 9.2, 8.3, 8.4_
  - [ ] 10.7 Implement `src/sync_state.rs`: `read_cursor(key) -> Option<i64>` and `write_cursor(key, value)` for sync cursor persistence
    - _Requirements: 9.3, 7.5, 8.5_
  - [ ] 10.8 Implement `src/search.rs`: FTS5 full-text search query (title + description, non-archived, ordered by `taken_at` DESC, limit 50); date range query (`taken_at BETWEEN`); geographic bounding-box query (`latitude/longitude BETWEEN`); FTS5 index rebuild via `INSERT INTO files_fts(files_fts) VALUES('rebuild')` when index is stale
    - _Requirements: 21.1–21.4, 21.6_
  - [ ]* 10.9 Write integration tests using `tempfile::TempDir`: open DB with SQLCipher key; migrations run in order; collections CRUD; files upsert and archive; sync_state read/write; FTS5 text search returns correct results; FTS5 index rebuild; date range query; geographic bounding box query; keychain unavailable returns `LocalDbError::KeychainUnavailable` without opening unencrypted DB
    - _Requirements: 9.1–9.7, 21.1–21.6_

- [ ] 11. Implement `crates/zoo` — Layer 2 Zoo server
  - [ ] 11.1 Create `crates/zoo/Cargo.toml` with `types`, `common`, `axum`, `tokio`, `sqlx` (postgres), `aws-sdk-s3`, `tower`, `serde`, `bcrypt`, `uuid`, `utoipa` dependencies; explicitly NO `crypto`, `image`, `metadata`, `thumbnail`, `sync`, `local-db`, `client-lib`
    - _Requirements: 24.3, 25.4_
  - [ ] 11.2 Write all PostgreSQL migrations (001–007) as defined in design §5.2: `users`, `sessions`, `devices`, `uploads` (with `parts_bitmask BYTEA`, dedup index including `collection_id`), `upload_parts`, `files`, `shares` tables with all indexes
    - _Requirements: 1.10, 2.5, 12.2, 13.5, 15.3, 6.4_
  - [ ] 11.3 Implement `src/config.rs`: `ZooConfig` loaded from environment variables (DB URL, S3 endpoint, stall timeout, GC interval, presign TTL, download mode)
    - _Requirements: 14.2, 16.1, 18.1, 20.7_
  - [ ] 11.4 Implement `src/db/` module: DB pool initialisation, migration runner, and all DB row structs (`UploadRow`, `FileRow`, `DeviceRow`, `UserRow`, `SessionRow`, `ShareRow`)
    - _Requirements: 1.10, 2.8_
  - [ ] 11.5 Implement `src/db/users.rs`: `register_user` (store `bcrypt(VerifyKeyHash)`, never plaintext), `find_user_by_email`, `get_user_key_attributes`
    - _Requirements: 1.10, 2.1, 2.5_
  - [ ] 11.6 Implement `src/db/sessions.rs`: `create_session` (store `SHA-256(SessionToken)`), `lookup_session` (hash incoming token, look up user_id), `revoke_session`
    - _Requirements: 2.5, 2.8, 2.10_
  - [ ] 11.7 Implement `src/db/devices.rs`: `register_device` (generate UUID device_id and unique SseToken), `tombstone_device` (set `archived_at`), `lookup_by_sse_token`, `get_device_stall_timeout`
    - _Requirements: 12.1, 12.2, 12.6, 16.5_
  - [ ] 11.8 Implement `src/db/uploads.rs`: `create_upload`, `patch_upload_status`, `update_bitmask`, `update_heartbeat`, `list_uploads_by_status`, `get_upload`; enforce dedup index (user_id + file_hash + collection_id for active statuses)
    - _Requirements: 12.4, 12.5, 13.4, 13.5, 28.1–28.4_
  - [ ] 11.9 Implement `src/db/upload_parts.rs`: `insert_parts_batch`, `mark_part_uploaded`, `list_pending_parts`
    - _Requirements: 14.1_
  - [ ] 11.10 Implement `src/db/files.rs`: `insert_file_record`, `get_file_for_download`, `list_files_for_sync`, `archive_file`
    - _Requirements: 15.3, 20.1, 7.2_
  - [ ] 11.11 Implement `src/state.rs`: upload state machine transition validator — `validate_transition(from: UploadStatus, to: UploadStatus) -> Result<(), ApiError>`; encode the full allowed-transitions table from design §5.3; reject `STALLED → ENCRYPTING`; return HTTP 400 for invalid transitions, HTTP 409 for conflicts
    - _Requirements: 13.1, 13.2, 13.3, 13.7_
  - [ ] 11.12 Implement `src/s3/`: S3 client initialisation, `presigner.rs` (presigned PUT/GET URL generation with configurable TTL), `client.rs` (HeadObject, ListParts, AbortMultipartUpload, DeleteObject)
    - _Requirements: 14.1, 14.2, 15.1, 18.2, 18.3, 27.2, 27.3_
  - [ ] 11.13 Implement `src/sse/hub.rs`: `SseHub` with `RwLock<HashMap<String, broadcast::Sender<SseEvent>>>`, buffer capacity 256; `subscribe`, `broadcast`, `cleanup_if_empty` methods; slow consumers are dropped (lagged) rather than blocking
    - _Requirements: 19.6_
  - [ ] 11.14 Implement `src/sse/events.rs`: `SseEvent` serialisation to `text/event-stream` format; Postgres LISTEN/NOTIFY background task for multi-replica fan-out
    - _Requirements: 19.7_
  - [ ] 11.15 Implement `src/auth/middleware.rs`: Tower layer extracting `Authorization: Bearer <token>`, hashing it with SHA-256, looking up user_id in sessions table; return HTTP 401 for invalid/expired tokens
    - _Requirements: 2.8, 2.9_
  - [ ] 11.16 Implement `src/api/auth.rs`: `POST /api/auth/params` (return same Argon2id params regardless of email existence), `POST /api/auth/login` (constant-time bcrypt verify; dummy bcrypt for non-existent emails; generate 32-byte random SessionToken; return token + key_attributes), `DELETE /api/auth/logout`, `POST /api/auth/register` (store bcrypt hash; return 409 on duplicate email)
    - _Requirements: 1.9–1.12, 2.1–2.10_
  - [ ] 11.17 Implement `src/api/devices.rs`: `POST /api/devices` (register device, return device_id + SseToken; 409 on duplicate name), `PATCH /api/devices/me`, `DELETE /api/devices/me` (tombstone device, cancel pending uploads, invalidate SseToken)
    - _Requirements: 12.1–12.3, 12.6_
  - [ ] 11.18 Implement `src/api/uploads.rs`: full upload lifecycle endpoints — `POST /api/uploads` (create with PENDING status; 409 on dedup), `PATCH /api/uploads/{id}` (validate state transition, update bitmask, broadcast SSE), `POST /api/uploads/{id}/presign` (S3 CreateMultipartUpload, insert parts, return presigned URLs; validate part_count ≤ 10000 and part_size bounds), `POST /api/uploads/{id}/presign-refresh` (new URLs for pending parts), `POST /api/uploads/{id}/register` (HeadObject verify, insert file record, delete upload rows, broadcast `upload.done`; idempotent), `DELETE /api/uploads/{id}` (cancel: AbortMultipartUpload or DeleteObject, set FAILED, broadcast SSE), `GET /api/uploads`, `GET /api/uploads/{id}`
    - _Requirements: 12.4, 12.5, 13.1–13.7, 14.1–14.6, 15.1–15.6, 27.1–27.5, 28.1–28.5_
  - [ ] 11.19 Implement `src/api/files.rs`: `GET /api/files/{id}/download` (verify ownership/share, check archived_at, redirect mode: presigned S3 GET 302; proxy mode: stream bytes), `GET /api/files/{id}/thumbnail`
    - _Requirements: 20.1–20.7_
  - [ ] 11.20 Implement `src/api/sync.rs`: `GET /api/sync/collections?since=<cursor>` (version-consistent pagination: discard last group sharing same `updation_time` if incomplete, set `has_more=true`), `GET /api/sync/files?collection_id=&since=&limit=1000`, `GET /api/sync/trash?since=<cursor>`
    - _Requirements: 7.2, 7.3, 8.2, 8.7_
  - [ ] 11.21 Implement `src/api/events.rs`: `GET /api/events` (authenticate via SseToken; subscribe to SSE Hub; send `upload.pending` immediately on connect; stream events; send heartbeat every 15s; broadcast `device.connected`/`device.disconnected`; return 401 for invalid/archived SseToken)
    - _Requirements: 19.1–19.8_
  - [ ] 11.22 Implement `src/workers/stall_detector.rs`: 15-second loop; query `SELECT ... FOR UPDATE SKIP LOCKED` for UPLOADING/ENCRYPTING uploads with `last_heartbeat_at < NOW() - device.stall_timeout_seconds`; update status to STALLED, set `stalled_at`, reset `expires_at = NOW() + 7 days`; broadcast `upload.stalled` SSE; send push notification if `push_token` set
    - _Requirements: 16.1–16.5_
  - [ ] 11.23 Implement `src/workers/garbage_collector.rs`: 5-minute loop; query `SELECT ... FOR UPDATE SKIP LOCKED` for non-terminal uploads with `expires_at < NOW()`; AbortMultipartUpload or DeleteObject as appropriate; set status FAILED with `error_reason = 'gc_expired'`; broadcast `upload.failed` SSE
    - _Requirements: 18.1–18.7_
  - [ ] 11.24 Wire everything in `src/lib.rs` and `bin/zoo-server.rs`: load config, init DB pool, run migrations, init S3 client, init SseHub, spawn stall detector and GC worker tasks, start Postgres LISTEN/NOTIFY task, build Axum router with all routes and auth middleware, start server
    - _Requirements: 25.4_
  - [ ]* 11.25 Write integration tests (using `sqlx::test` with real PostgreSQL + MinIO): registration stores `bcrypt(VerifyKeyHash)` not plaintext; login with correct hash returns session token; login with wrong hash returns 401; login for non-existent email returns 401 with no timing difference; state machine rejects invalid transitions with 400; `STALLED→UPLOADING` allowed, `STALLED→ENCRYPTING` rejected with 400; `parts_bitmask` stored as big-endian bit vector; stall detector marks `UPLOADING→STALLED` after timeout; stall detector uses `SKIP LOCKED`; GC marks expired uploads FAILED and calls S3 abort; GC uses `SKIP LOCKED`; SSE hub broadcasts to all subscribers; Postgres LISTEN/NOTIFY fan-out; `POST /api/uploads/{id}/register` is idempotent; HeadObject size mismatch returns 400; dedup index prevents duplicate active uploads for same file+collection; share `expires_at` in past returns 403
    - _Requirements: 1.9–1.12, 2.1–2.10, 6.4–6.5, 12–19, 20, 24.3_
  - [ ]* 11.26 Write property test: `∀ rows, page_size: paginate(rows, page_size)` never splits a group of records sharing the same `updation_time` across two pages
    - **Property 7: Version-consistent pagination**
    - **Validates: Requirements 7.3**
  - [ ]* 11.27 Write property test: `∀ bitmask ∈ Vec<u8>: encode_bitmask(decode_bitmask(bitmask)) == bitmask` (big-endian bit vector round-trip)
    - **Property 8: Parts bitmask encoding round-trip**
    - **Validates: Requirements 13.5**
  - [ ]* 11.28 Write property test for upload state machine: `∀ sequence of valid transitions starting from PENDING: final state is always DONE or FAILED`; `∀ upload in DONE or FAILED: no further transition is accepted`; `STALLED → UPLOADING` is always accepted; `STALLED → ENCRYPTING` is always rejected
    - **Property 9: Upload state machine terminal states and STALLED resume invariant**
    - **Validates: Requirements 13.1, 13.3**

- [ ] 12. Implement `crates/zoo-client` — Layer 2 upload/download client SDK
  - [ ] 12.1 Create `crates/zoo-client/Cargo.toml` with `types`, `reqwest` (no default features, with `json` and `stream` features) dependencies; explicitly NO `crypto`, `image`, `metadata`, `thumbnail`
    - _Requirements: 25.6_
  - [ ] 12.2 Implement `src/lib.rs`: `ZooClient` struct with base URL and session token; constructor and per-request token injection
    - _Requirements: 25.6_
  - [ ] 12.3 Implement `src/orchestrator.rs`: `upload_file(client, source_bytes, metadata, file_key) -> Result<FileId, ZooError>` — full upload lifecycle: POST uploads → PATCH encrypting → compute part MD5s → POST presign → PATCH uploading → upload parts with heartbeat every 30s or every 5 parts → handle S3 403 by calling presign-refresh and retrying (max 3 attempts per part) → S3 complete → PATCH s3_completed → POST register; return `ZooError::S3Error` on persistent S3 failure
    - _Requirements: 12.4, 13.1, 13.6, 14.1–14.6, 15.1, 15.5_
  - [ ] 12.4 Implement `src/orchestrator.rs` resume path: `resume_upload(client, upload_id, source_bytes) -> Result<FileId, ZooError>` — PATCH resuming (STALLED→UPLOADING) → GET upload state → S3 ListParts reconciliation (all 5 cases from design §9.3) → POST presign-refresh → upload missing parts → complete + register; handle `NoSuchUpload` by marking FAILED and returning error
    - _Requirements: 13.3, 17.1–17.7_
  - [ ] 12.5 Implement `src/upload.rs`: `s3_put_part(url, bytes) -> Result<ETag, ZooError>` and `s3_complete(complete_url, etags) -> Result<(), ZooError>`
    - _Requirements: 14.6_
  - [ ] 12.6 Implement `src/download.rs`: `download_file(file_id) -> Result<Vec<u8>, ZooError>` (follow 302 redirect or stream proxy bytes), `get_thumbnail(file_id) -> Result<Vec<u8>, ZooError>`
    - _Requirements: 20.4, 20.5, 11.4_
  - [ ] 12.7 Implement `src/sse.rs`: SSE event stream client; parse `text/event-stream` into `SseEvent` variants; reconnect with exponential backoff on disconnect
    - _Requirements: 19.1, 17.1_
  - [ ]* 12.8 Write integration tests using `wiremock` (Zoo mock) and `mockito` (S3 mock): full upload flow POST→PATCH→presign→PUT parts→PATCH s3_completed→POST register; heartbeat sent every 30s; 403 from S3 triggers presign-refresh and retry; resume from STALLED transitions to UPLOADING (not ENCRYPTING); S3 ListParts reconciliation all 5 cases; `NoSuchUpload` marks FAILED
    - _Requirements: 13.3, 14.1–14.6, 17.1–17.7_

- [ ] 13. Implement `crates/sync` — Layer 2 incremental sync engine
  - [ ] 13.1 Create `crates/sync/Cargo.toml` with `types`, `crypto`, `metadata`, `thumbnail`, `local-db`, `common`, `zoo-client` dependencies
    - _Requirements: 25.4_
  - [ ] 13.2 Implement `src/cursor.rs`: `read(key) -> Option<i64>` and `write(key, value)` delegating to `local_db::sync_state`
    - _Requirements: 7.5, 8.5_
  - [ ] 13.3 Implement `src/decrypt.rs`: `batch_decrypt_files(records, collection_key) -> Result<Vec<FileRecord>, SyncError>` — for each record: `secretbox_decrypt` FileKey with CollectionKey → `stream_decrypt` metadata with FileKey → `serde_json::from_slice` → build `FileRecord`; on decryption failure log error, skip record, continue (non-fatal)
    - _Requirements: 8.3, 4.6, 7.7_
  - [ ] 13.4 Implement `src/diff.rs`: `fetch_collection_page(since) -> Result<SyncCollectionResponse, SyncError>` and `fetch_file_page(collection_id, since, limit) -> Result<SyncFilesResponse, SyncError>` and `fetch_trash_page(since) -> Result<SyncTrashResponse, SyncError>`
    - _Requirements: 7.1, 8.1, 8.7_
  - [ ] 13.5 Implement `src/pull.rs`: `sync_all(engine) -> Result<(), SyncError>` — Step 1: sync collections (loop with `has_more`, persist cursor after each page); Step 2: sync files per collection (loop with `has_more`, persist cursor); Step 3: sync trash; Step 4: queue thumbnail downloads for new files; offline: return immediately without network calls
    - _Requirements: 7.1–7.7, 8.1–8.8_
  - [ ] 13.6 Implement `src/thumbnails.rs`: `queue_new_files(local_db) -> Result<(), SyncError>` — list files without `thumbnail_path`, enqueue thumbnail download tasks
    - _Requirements: 11.4_
  - [ ]* 13.7 Write integration tests using `wiremock`: `sync_all()` fetches collections then files then trash in order; cursor persisted after each page; `has_more=true` triggers next page fetch; decryption failure skips record and continues; version-consistent pagination discards incomplete last group; offline mode serves from local DB without network calls
    - _Requirements: 7.1–7.7, 8.1–8.8_
  - [ ]* 13.8 Write property test: `∀ page_size, collection of records with varying updation_times: sync_all() with paginated responses always advances cursor monotonically and never processes the same record twice`
    - **Property 10: Sync cursor monotonicity**
    - **Validates: Requirements 7.5, 8.5**

- [ ] 14. Checkpoint — Layer 2 complete
  - Ensure `cargo test -p local-db -p zoo -p zoo-client -p sync` passes. Ask the user if questions arise.

- [ ] 15. Implement `crates/client-lib` — Layer 3 Tauri command layer
  - [ ] 15.1 Create `crates/client-lib/Cargo.toml` with `types`, `sync`, `local-db`, `thumbnail`, `zoo-client`, `tauri` (behind `desktop` feature flag), `tauri-specta`, `specta`, `tokio` dependencies; explicitly NO `axum`, `sqlx` (postgres), `aws-sdk-s3`
    - _Requirements: 25.5_
  - [ ] 15.2 Implement `src/state.rs`: `AppState` struct with `Arc<LocalDb>`, `Arc<RwLock<Option<Key256>>>` (master_key), `Arc<RwLock<Option<String>>>` (session_token), `Arc<RwLock<Option<DeviceInfo>>>`, `Arc<RwLock<SyncState>>`, `Arc<ThumbnailCache>`, `Arc<ZooClient>`, `Arc<AppConfig>`; `AppState::init(config)` initialises all fields
    - _Requirements: 22.2_
  - [ ] 15.3 Implement `src/commands/auth.rs`: `get_auth_params`, `login` (derive KEK → decrypt MasterKey → store in secret memory), `logout` (zeroize MasterKey, revoke session), `register` (full key bootstrapping: derive KEK, generate MasterKey, encrypt keys, derive VerificationKey, compute VerifyKeyHash, POST register)
    - _Requirements: 1.1–1.12, 2.1–2.10, 22.3_
  - [ ] 15.4 Implement `src/commands/collections.rs`: `list_collections`, `create_collection` (generate CollectionKey, encrypt with MasterKey, POST to Zoo), `archive_collection`
    - _Requirements: 4.1, 4.2, 22.3_
  - [ ] 15.5 Implement `src/commands/files.rs`: `list_files`, `get_file`, `archive_file`, `download_file` (call zoo-client download, decrypt with FileKey using stored `file_decryption_header`, write to destination path)
    - _Requirements: 20.6, 22.3_
  - [ ] 15.6 Implement `src/commands/upload.rs`: `upload_file` (read file, extract EXIF, generate thumbnail, encrypt file + metadata + thumbnail, call zoo-client orchestrator), `cancel_upload`, `list_pending_uploads`
    - _Requirements: 5.1–5.3, 10.1–10.8, 26.1–26.5, 22.3_
  - [ ] 15.7 Implement `src/commands/sync.rs`: `trigger_sync` (check connectivity, call `sync::sync_all`), `get_sync_status`
    - _Requirements: 8.8, 22.2, 22.3_
  - [ ] 15.8 Implement `src/commands/thumbnails.rs`: `get_thumbnail` (delegate to `ThumbnailCache::get`), `evict_thumbnail`
    - _Requirements: 11.2–11.4, 22.3_
  - [ ] 15.9 Implement `src/commands/device.rs`: `register_device` (POST /api/devices, store device_id and SseToken in AppState), `get_device_info`
    - _Requirements: 12.1, 12.2, 22.3_
  - [ ] 15.10 Implement `src/commands/search.rs`: `search_files` (FTS5 query via local-db), `search_by_date`, `search_by_location`
    - _Requirements: 21.1–21.4, 22.3_
  - [ ] 15.11 Add `#[tauri::command]` and `#[specta::specta]` attributes to all commands; register all commands in `src/commands/mod.rs`
    - _Requirements: 22.3, 22.4_
  - [ ]* 15.12 Write integration tests using mock dependencies: `login` command decrypts MasterKey and stores in AppState; `upload_file` command encrypts file and calls zoo-client; `get_thumbnail` returns from memory cache on hit; `search_files` executes FTS5 query; `trigger_sync` calls `sync_all`; MasterKey is zeroized and absent from AppState after logout
    - _Requirements: 22.2–22.7_

- [ ] 16. Implement `crates/zoo-wasm` — Layer 3 WASM bindings
  - [ ] 16.1 Create `crates/zoo-wasm/Cargo.toml` with `types`, `zoo-client`, `wasm-bindgen`, `wasm-bindgen-futures`, `serde-wasm-bindgen` dependencies; explicitly NO `crypto`, `image`, `metadata`, `thumbnail`
    - _Requirements: 23.1, 24.4, 25.4_
  - [ ] 16.2 Implement `src/lib.rs`: `ZooHandle` struct with `#[wasm_bindgen]` — `create(config: JsValue)`, `upload_file(encrypted_bytes, metadata: JsValue)`, `pending_uploads()`, `cancel_upload(upload_id)`, `close()`; all async methods return `Result<JsValue, JsError>`
    - _Requirements: 23.1, 23.3_
  - [ ] 16.3 Add `wasm-pack build crates/zoo-wasm --target web --out-dir apps/web/src/wasm` to `Makefile` and CI
    - _Requirements: 23.1_
  - [ ]* 16.4 Write unit tests for WASM bindings: `ZooHandle::create` initialises client; `upload_file` delegates to zoo-client orchestrator; `cancel_upload` calls DELETE endpoint
    - _Requirements: 23.1, 23.3_

- [ ] 17. Checkpoint — Layer 3 complete
  - Ensure `cargo test -p client-lib -p zoo-wasm` passes. Ask the user if questions arise.

- [ ] 18. Scaffold `apps/desktop` — Tauri desktop application
  - [ ] 18.1 Create `apps/desktop/src-tauri/Cargo.toml` with `tauri`, `client-lib` dependencies; no business logic
    - _Requirements: 22.1_
  - [ ] 18.2 Implement `apps/desktop/src-tauri/src/main.rs`: `AppState::init` → `tauri::Builder::default().manage(state).invoke_handler(tauri::generate_handler![...all commands...]).run()`
    - _Requirements: 22.1, 22.2_
  - [ ] 18.3 Implement `apps/desktop/src-tauri/src/bindings.rs` (build-time codegen): `tauri_specta::Builder` collecting all commands, exporting to `apps/desktop/src/bindings.ts`
    - _Requirements: 22.4_
  - [ ] 18.4 Create `apps/desktop/tauri.conf.json` with app identifier, window config, and allowed API permissions
    - _Requirements: 22.1_
  - [ ] 18.5 Create `apps/desktop/src/` React frontend scaffold: `app/` directory with App Router pages (login, gallery, upload), `components/` directory, import of generated `bindings.ts`
    - _Requirements: 22.3, 22.7_
  - [ ]* 18.6 Verify `cargo tauri build --target current` compiles without errors on the host platform; verify `bindings.ts` is generated and TypeScript compilation succeeds
    - _Requirements: 22.1, 22.4_

- [ ] 19. Scaffold `apps/web` — Next.js web application
  - [ ] 19.1 Create `apps/web/package.json` with Next.js, React, and `packages/ui` dependency; create `apps/web/next.config.ts` with WASM support (`asyncWebAssembly: true`)
    - _Requirements: 23.1_
  - [ ] 19.2 Create `apps/web/src/app/` App Router pages: login, gallery, upload; import WASM module from `src/wasm/` (generated by `wasm-pack`)
    - _Requirements: 23.2, 23.5_
  - [ ] 19.3 Implement upload resume in web: persist `upload_id` in `localStorage` on upload start; on page load, check `localStorage` for pending upload_id; reconnect SSE stream and display stalled upload state; prompt user to resume if File System Access API handle is available
    - _Requirements: 23.4, 17.7_
  - [ ] 19.4 Add OpenAPI TypeScript client generation to `Makefile`: `cargo run --bin gen-openapi > openapi.json` then `pnpm openapi-typescript openapi.json -o packages/api-client/src/generated.ts`; add `utoipa` annotations to all Zoo handlers
    - _Requirements: 23.6_
  - [ ] 19.5 Create `packages/ui/` shared React component library (shadcn-based): photo grid, upload progress bar, thumbnail component, collection list; used by both `apps/desktop/src/` and `apps/web/src/`
    - _Requirements: 23.7_
  - [ ]* 19.6 Verify `pnpm turbo build` compiles the Next.js app without errors; verify generated TypeScript client compiles against the OpenAPI spec
    - _Requirements: 23.6_

- [ ] 20. Final checkpoint — Full workspace
  - Ensure `cargo test --workspace --all-features` passes, `cargo clippy --workspace -- -D warnings` is clean, and `pnpm turbo test` passes. Ask the user if questions arise.

---

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP build
- Each task references specific requirements for full traceability
- Checkpoints at tasks 4, 9, 14, 17, and 20 validate each layer before proceeding
- Property tests use `proptest` crate; unit tests use the standard `#[test]` harness
- Integration tests for `zoo` require Docker (PostgreSQL + MinIO); use `sqlx::test` macro
- Integration tests for `local-db` use `tempfile::TempDir` for isolated SQLite databases
- Integration tests for `sync` and `zoo-client` use `wiremock` for HTTP mocking
- The `zoo` crate must never import `crypto`, `image`, `metadata`, or `thumbnail` — enforced at compile time by Cargo
- The `zoo-wasm` crate must never import `crypto`, `image`, `metadata`, or `thumbnail` — enforced at compile time
- MasterKey is held in `Arc<RwLock<Option<Key256>>>` with `ZeroizeOnDrop`; never written to disk
- All presigned URL expiry handling (HTTP 403 from S3) is handled transparently inside `zoo-client::orchestrator`
- The `parts_bitmask` is a big-endian bit vector: bit N (0-indexed from MSB of byte 0) = part N


## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1"] },
    { "id": 1, "tasks": ["2.1", "3.1"] },
    { "id": 2, "tasks": ["2.2", "2.3", "2.4", "2.5", "2.6", "2.7", "2.8", "3.2"] },
    { "id": 3, "tasks": ["2.9", "3.3"] },
    { "id": 4, "tasks": ["5.1", "6.1", "7.1", "8.1"] },
    { "id": 5, "tasks": ["5.2", "5.3", "5.4", "5.5", "5.6", "5.7", "5.8", "5.9", "6.2", "6.3", "6.4", "6.5", "6.6"] },
    { "id": 6, "tasks": ["5.10", "5.11", "5.12", "5.13", "5.14", "6.7", "7.2", "8.2", "8.3"] },
    { "id": 7, "tasks": ["7.3", "7.4", "8.4", "8.5", "8.6", "8.7", "8.8"] },
    { "id": 8, "tasks": ["8.9", "8.10"] },
    { "id": 9, "tasks": ["10.1", "11.1", "12.1", "13.1"] },
    { "id": 10, "tasks": ["10.2", "10.3", "11.2", "11.3", "11.4", "12.2", "13.2"] },
    { "id": 11, "tasks": ["10.4", "10.5", "10.6", "10.7", "10.8", "11.5", "11.6", "11.7", "11.8", "11.9", "11.10", "11.11", "11.12", "11.13", "11.14", "11.15", "12.3", "12.5", "12.6", "12.7", "13.3", "13.4"] },
    { "id": 12, "tasks": ["10.9", "11.16", "11.17", "11.18", "11.19", "11.20", "11.21", "11.22", "11.23", "12.4", "13.5", "13.6"] },
    { "id": 13, "tasks": ["11.24", "11.25", "11.26", "11.27", "11.28", "12.8", "13.7", "13.8"] },
    { "id": 14, "tasks": ["15.1", "16.1"] },
    { "id": 15, "tasks": ["15.2", "15.3", "15.4", "15.5", "15.6", "15.7", "15.8", "15.9", "15.10", "16.2", "16.3"] },
    { "id": 16, "tasks": ["15.11", "16.4"] },
    { "id": 17, "tasks": ["15.12"] },
    { "id": 18, "tasks": ["18.1", "19.1"] },
    { "id": 19, "tasks": ["18.2", "18.3", "18.4", "18.5", "19.2", "19.3", "19.4", "19.5"] },
    { "id": 20, "tasks": ["18.6", "19.6"] }
  ]
}
```
