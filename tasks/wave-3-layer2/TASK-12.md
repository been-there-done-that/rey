# Task 12: Implement `crates/zoo-client` — Layer 2 Upload/Download Client SDK

## Wave
3 (Layer 2 — Application Logic)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete

## Can Run In Parallel With
- Task 10 (local-db), Task 11 (zoo) — no dependencies between them

## Design References
- design.md §9.1: Zoo Client Module Structure
- design.md §9.2: Upload Orchestrator
- design.md §9.3: Resume Protocol
- design.md §9.4: Presigned URL Expiry Handling
- ZOO.md §13: Client SDK

## Requirements
12.4, 13.1–13.7, 14.1–14.6, 15.1–15.6, 17.1–17.7, 19.1, 20.4–20.5, 23.3–23.4, 25.6, 27.5, 28.5

## Objective
Platform-agnostic upload/download state machine. No crypto — works with encrypted bytes only. HTTP client via reqwest.

## Cargo.toml
```toml
[package]
name = "zoo-client"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
reqwest = { workspace = true }
thiserror = { workspace = true }
serde_json = { workspace = true }
base64 = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod orchestrator;
pub mod upload;
pub mod download;
pub mod sse;
pub mod error;

pub use orchestrator::ZooClient;
```

### `src/lib.rs` — ZooClient struct
```rust
pub struct ZooClient {
    base_url: String,
    session_token: Option<String>,
    client: reqwest::Client,
}
```
- Constructor: `new(base_url: String) -> Self`
- `set_session_token(&mut self, token: String)`
- Per-request `Authorization: Bearer <token>` injection via reqwest builder

### `src/orchestrator.rs` — Upload path
`upload_file(source_bytes: &[u8], metadata: UploadMetadata) -> Result<FileId, ZooError>`:
1. POST /api/uploads → get upload_id (409 = duplicate)
2. PATCH {status: "encrypting"}
3. Compute part MD5s (split into DEFAULT_PART_SIZE chunks)
4. POST /api/uploads/{id}/presign → get presigned URLs
5. PATCH {status: "uploading", parts_bitmask: empty}
6. Upload parts with heartbeat every 30s or every 5 parts
   - Handle S3 403 by calling presign-refresh and retrying (max 3 attempts per part)
7. S3 CompleteMultipartUpload
8. PATCH {status: "s3_completed"}
9. POST /api/uploads/{id}/register → get file_id (idempotent)

### `src/orchestrator.rs` — Resume path
`resume_upload(upload_id: Uuid, source_bytes: &[u8]) -> Result<FileId, ZooError>`:
1. PATCH {status: "resuming"} (STALLED→UPLOADING)
2. GET /api/uploads/{id} → get current state
3. S3 ListParts reconciliation (all 5 cases from design.md §9.3 table)
4. POST /api/uploads/{id}/presign-refresh
5. Upload missing parts (same heartbeat logic)
6. Complete + register
7. Handle NoSuchUpload → mark FAILED, return error

### `src/upload.rs`
- `s3_put_part(url: &str, bytes: &[u8]) -> Result<ETag, ZooError>` — HTTP PUT to presigned URL
- `s3_complete(complete_url: &str, etags: &[ETag]) -> Result<(), ZooError>` — POST to presigned complete URL

### `src/download.rs`
- `download_file(file_id: i64) -> Result<Vec<u8>, ZooError>` — GET /api/files/{id}/download, follow 302 redirect or stream proxy
- `get_thumbnail(file_id: i64) -> Result<Vec<u8>, ZooError>` — GET /api/files/{id}/thumbnail

### `src/sse.rs`
- SSE event stream client: connect to GET /api/events
- Parse `text/event-stream` into `SseEvent` variants
- Reconnect with exponential backoff on disconnect (1s, 2s, 4s, 8s, max 30s)

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum ZooError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("S3 error: {0}")]
    S3Error(String),
    #[error("upload was aborted by GC or manual action")]
    UploadAborted,
    #[error("invalid state transition: {0}")]
    StateError(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("conflict: {0}")]
    Conflict(String),
}
```

## Tests (Task 12.8 — marked with *)
Integration tests using `wiremock` (Zoo mock) and `mockito` (S3 mock):
- Full upload flow: POST→PATCH→presign→PUT parts→PATCH s3_completed→POST register
- Heartbeat sent every 30s
- 403 from S3 triggers presign-refresh and retry
- Resume from STALLED transitions to UPLOADING (not ENCRYPTING)
- S3 ListParts reconciliation all 5 cases
- NoSuchUpload marks FAILED

## Verification Steps
- [ ] `cargo check -p zoo-client` succeeds
- [ ] `cargo test -p zoo-client` passes
- [ ] NO crypto/image/metadata/thumbnail in `cargo tree -p zoo-client`
- [ ] Upload orchestrator handles all error cases
- [ ] SSE reconnect with exponential backoff works
- [ ] Presigned URL expiry (403) triggers transparent refresh

## Notes
- This crate has NO crypto — it works with already-encrypted bytes.
- The `reqwest` dependency uses `no default features` with `json` and `stream` features.
- The orchestrator is async — all methods are `async fn`.
- Heartbeat: send PATCH every 30s OR every 5 parts, whichever comes first.
- The retry limit for presign-refresh is 3 attempts per part to prevent infinite loops.
