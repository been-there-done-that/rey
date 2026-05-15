# Task 11: Implement `crates/zoo` — Layer 2 Zoo Server

## Wave
3 (Layer 2 — Application Logic)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 3 (common) must be complete

## Can Run In Parallel With
- Task 10 (local-db), Task 12 (zoo-client) — no dependencies between them
- Task 13 (sync) depends on all of Wave 2 + Task 10

## Design References
- design.md §5.1: Zoo Module Structure
- design.md §5.2: Database Schema (all 7 migrations)
- design.md §5.3: Upload State Machine (transition table)
- design.md §5.4: API Routes (all endpoints)
- design.md §5.5: SSE Hub Design
- design.md §5.6: Stall Detector
- design.md §5.7: GC Worker
- design.md §5.8: Auth Flow (registration + login)
- design.md §5.9: Rate Limiting
- design.md §5.10: Input Validation
- design.md §5.11: Configuration
- design.md §6.3: Version-Consistent Pagination
- design.md §12.1: Zero-Knowledge Guarantee
- design.md §12.2: Timing Attack Mitigations
- ZOO.md: Full service specification

## Requirements
1.9–1.12, 2.1–2.10, 6.4–6.5, 12–20, 24.3, 25.4, 27–28

## Objective
Implement the complete Zoo server: Axum HTTP API, PostgreSQL database, S3 storage, SSE hub, stall detection, garbage collection, authentication, rate limiting, input validation.

## CRITICAL CONSTRAINT
- **NO `crypto`, `image`, `metadata`, `thumbnail`, `sync`, `local-db`, `client-lib` dependencies** — enforced at compile time by Cargo (Req 24.3)

## Cargo.toml
```toml
[package]
name = "zoo"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[features]
default = ["s3"]
s3 = []
local-fs = []

[dependencies]
types = { workspace = true }
common = { workspace = true }
axum = { workspace = true }
tokio = { workspace = true }
sqlx = { workspace = true }
aws-sdk-s3 = { workspace = true }
aws-config = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
bcrypt = { workspace = true }
uuid = { workspace = true }
utoipa = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
dashmap = { workspace = true }
base64 = { workspace = true }
sha2 = "0.10"
rand = "0.8"
chrono = "0.4"
```

## Module Structure (create ALL files)

```
crates/zoo/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── types.rs
│   ├── state.rs
│   ├── error.rs
│   ├── rate_limit.rs
│   ├── validation.rs
│   ├── db/
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   ├── users.rs
│   │   ├── sessions.rs
│   │   ├── devices.rs
│   │   ├── uploads.rs
│   │   ├── upload_parts.rs
│   │   └── files.rs
│   ├── s3/
│   │   ├── mod.rs
│   │   ├── presigner.rs
│   │   └── client.rs
│   ├── sse/
│   │   ├── mod.rs
│   │   ├── hub.rs
│   │   └── events.rs
│   ├── workers/
│   │   ├── mod.rs
│   │   ├── stall_detector.rs
│   │   └── garbage_collector.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── auth.rs
│   │   ├── devices.rs
│   │   ├── uploads.rs
│   │   ├── files.rs
│   │   ├── sync.rs
│   │   └── events.rs
│   └── auth/
│       ├── mod.rs
│       └── middleware.rs
├── bin/
│   └── zoo-server.rs
└── migrations/
    ├── 001_create_users.sql
    ├── 002_create_sessions.sql
    ├── 003_create_devices.sql
    ├── 004_create_uploads.sql
    ├── 005_create_upload_parts.sql
    ├── 006_create_files.sql
    └── 007_create_shares.sql
```

## Implementation Details (summarized — each sub-module needs full implementation)

### `migrations/` — All 7 SQL files
Use the EXACT DDL from design.md §5.2. Key points:
- `users.verify_key_hash` stores `bcrypt(SHA-256(VerificationKey))`
- `sessions.token_hash` stores `SHA-256(SessionToken)`
- `uploads.parts_bitmask` is `BYTEA` with big-endian bit vector encoding
- `uploads` has dedup index: `UNIQUE (user_id, file_hash, metadata->>'collection_id') WHERE status IN ('pending','encrypting','uploading')`
- `files.updation_time` is `TIMESTAMPTZ` with index for pagination
- `shares` table for v2 sharing

### `src/config.rs`
`ZooConfig` loaded from env vars (see design.md §5.11 for full table):
- `LISTEN_ADDR`, `DATABASE_URL`, `S3_ENDPOINT`, `S3_REGION`, `S3_BUCKET`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`
- `SESSION_TTL_DAYS=30`, `DOWNLOAD_MODE=redirect`, `STALL_TIMEOUT_SECONDS=90`, `PRESIGNED_TTL_HOURS=24`, `GC_INTERVAL_SECONDS=300`, `MAX_FILE_SIZE=10737418240`, `DEFAULT_PART_SIZE=20971520`
- `DownloadMode` enum: `Redirect { presigned_ttl: Duration }`, `Proxy { max_concurrent: usize }`

### `src/state.rs`
Upload state machine validator:
- `validate_transition(from: UploadStatus, to: UploadStatus) -> Result<(), ApiError>`
- Allowed transitions from design.md §5.3 table
- `STALLED → UPLOADING` allowed, `STALLED → ENCRYPTING` rejected
- Return HTTP 400 for invalid transitions

### `src/db/` — All database modules
Use `sqlx` with `PgPool`. Each module handles one table:
- `users.rs`: `register_user`, `find_user_by_email`, `get_user_key_attributes`
- `sessions.rs`: `create_session` (store SHA-256 hash), `lookup_session`, `revoke_session`
- `devices.rs`: `register_device`, `tombstone_device`, `lookup_by_sse_token`, `get_device_stall_timeout`
- `uploads.rs`: `create_upload`, `patch_upload_status`, `update_bitmask`, `update_heartbeat`, `list_uploads_by_status`, `get_upload`
- `upload_parts.rs`: `insert_parts_batch`, `mark_part_uploaded`, `list_pending_parts`
- `files.rs`: `insert_file_record`, `get_file_for_download`, `list_files_for_sync` (with version-consistent pagination), `archive_file`

### `src/s3/` — S3 operations
- `presigner.rs`: Generate presigned PUT URLs for multipart parts, presigned CompleteMultipartUpload URL, presigned GET URLs for downloads
- `client.rs`: `head_object` (verify file size), `list_parts` (for resume reconciliation), `abort_multipart_upload`, `delete_object`

### `src/sse/hub.rs`
`SseHub` with `RwLock<HashMap<String, broadcast::Sender<SseEvent>>>`, buffer 256:
- `subscribe(user_id) → Receiver<SseEvent>`
- `broadcast(user_id, event)`
- `cleanup_if_empty(user_id)`
- Postgres LISTEN/NOTIFY for multi-replica fan-out

### `src/workers/`
- `stall_detector.rs`: 15s loop, `SELECT ... FOR UPDATE SKIP LOCKED`, mark STALLED, broadcast SSE
- `garbage_collector.rs`: 5m loop, `SELECT ... FOR UPDATE SKIP LOCKED`, abort S3 multipart, mark FAILED, broadcast SSE

### `src/api/` — All HTTP handlers
Implement ALL endpoints from design.md §5.4 table. Key handlers:
- `auth.rs`: POST /api/auth/params (anti-enumeration), POST /api/auth/login (dummy bcrypt for non-existent emails), POST /api/auth/register, DELETE /api/auth/logout
- `uploads.rs`: Full lifecycle — POST, PATCH, presign, presign-refresh, register, DELETE, GET
- `files.rs`: GET /api/files/{id}/download (redirect or proxy), GET /api/files/{id}/thumbnail
- `sync.rs`: GET /api/sync/collections, /files, /trash (version-consistent pagination)
- `events.rs`: GET /api/events (SSE stream)

### `src/auth/middleware.rs`
Tower layer: extract `Authorization: Bearer <token>`, SHA-256 hash, lookup user_id in sessions, return 401 for invalid/expired

### `src/rate_limit.rs`
Tower middleware with `DashMap<String, RateLimitState>` keyed by user_id:
- POST /api/uploads: 100/hr
- POST .../presign: 50/upload
- PATCH /api/uploads/{id}: 1 per 5 seconds
- GET /api/files/{id}/download: 1000/hr
- GET /api/events: 1 concurrent/device
- Return HTTP 429 with `Retry-After` header

### `src/validation.rs`
Request validation extractors:
- file_size ≤ 10 GiB, part_size 5 MiB–5 GiB, part_count ≤ 10000
- part_md5s match part_count, each decodes to 16 bytes
- email format ≤ 255 chars, device name ≤ 64 chars no null bytes
- Return HTTP 400 `validation_error`

### `bin/zoo-server.rs`
Wire everything: load config, init PgPool, run migrations, init S3 client, init SseHub, spawn workers, build Axum router with all routes + auth middleware + rate limiting, start server

## Tests (Tasks 11.27–11.30 — marked with *)

### Integration Tests (11.27)
Using `sqlx::test` with real PostgreSQL + MinIO:
- Registration stores bcrypt hash, not plaintext
- Login with correct hash returns session token
- Login with wrong hash returns 401
- Login for non-existent email returns 401 with no timing difference
- State machine rejects invalid transitions with 400
- STALLED→UPLOADING allowed, STALLED→ENCRYPTING rejected
- parts_bitmask stored as big-endian bit vector
- Stall detector marks UPLOADING→STALLED after timeout
- GC marks expired uploads FAILED and calls S3 abort
- SSE hub broadcasts to all subscribers
- Postgres LISTEN/NOTIFY fan-out
- POST /api/uploads/{id}/register is idempotent
- HeadObject size mismatch returns 400
- Dedup index prevents duplicate active uploads
- Rate limiting returns 429 with Retry-After
- Input validation returns 400 for invalid inputs
- API error responses follow standard JSON schema

### Property Tests (11.28–11.30)
- **Property 7**: Version-consistent pagination never splits same updation_time across pages
- **Property 8**: Bitmask encoding round-trip
- **Property 9**: State machine terminal states invariant

## Verification Steps
- [ ] `cargo check -p zoo` succeeds
- [ ] `cargo test -p zoo` passes (requires Docker with PostgreSQL + MinIO)
- [ ] NO crypto/image/metadata/thumbnail in `cargo tree -p zoo`
- [ ] All API endpoints return correct status codes
- [ ] Rate limiting works under load
- [ ] SSE events are broadcast to all connected clients
- [ ] Stall detector and GC workers run on schedule
- [ ] Version-consistent pagination works correctly

## Notes
- This is the largest single task. Consider splitting across multiple agents if needed:
  - Agent A: migrations + db modules + config + state
  - Agent B: api handlers (auth, devices, uploads)
  - Agent C: api handlers (files, sync, events) + sse + s3
  - Agent D: workers + rate_limit + validation + bin/zoo-server.rs
- The `sqlx::test` macro requires `DATABASE_URL` env var pointing to a test PostgreSQL instance.
- MinIO can be run as a Docker container for S3 testing.
- The `DUMMY_BCRYPT_HASH` constant is a pre-computed bcrypt hash for timing attack mitigation.
- All error responses follow the standard JSON schema: `{"error": {"code": "...", "message": "...", "details": {...}}}`.
