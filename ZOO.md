# Zoo — Upload Reliability & File Delivery Service

## 1. Purpose

Zoo is a standalone Rust service that owns the entire upload and download
pipeline for encrypted files. It replaces the fire-and-forget model of
"here's a presigned URL, good luck" with a stateful protocol that detects
failure, enables resume, provides real-time progress, and never orphans
objects in storage.

Zoo is the only server in the system. There is no separate metadata service.
Zoo stores file records, manages S3 storage, handles access control, delivers
downloads, and broadcasts real-time events.

---

## 2. Problem Space

What goes wrong with the naive presigned-URL approach:

| Failure | Naive outcome | Zoo outcome |
|---------|---------------|-------------|
| Client drops mid-upload | Object orphaned until GC (days/weeks) | SSE disconnect detected in seconds, upload marked STALLED, push notification sent |
| Client dies after S3 completion but before server registration | File in S3, no record in DB | Registration is idempotent; client retries on reconnect |
| Web tab closes during upload | Entire upload lost | Upload ID persisted in localStorage; resume on next visit |
| User switches devices mid-upload | No visibility into what's pending | All devices receive real-time SSE events; queue visible everywhere |
| Large file takes hours | No progress visible anywhere | SSE pushes parts_bitmask to all connected devices |
| Clock skew breaks presigned URL | S3 rejects with 403, user retries blindly | Heartbeat mechanism refreshes URLs |
| Concurrent upload of same file from 2 devices | Duplicate objects, wasted bandwidth | UNIQUE constraint on file_hash per user; second device warned |
| Self-hosted user reboots server during upload | ListParts from S3 is ground truth | Cross-referenced bitmask vs S3, reconcile on resume |
| User has no idea what their other devices are doing | No visibility | Device registry + named devices + SSE events |

---

## 3. Architectural Constraints

- **Zero-knowledge encryption**: Zoo never sees plaintext. Encryption happens
  client-side. Keys are derived client-side and stored encrypted in Zoo's DB.
- **Source file locality**: The plaintext source file exists only on the
  originating device. Cross-device partial resume is impossible — the source
  is not available on the second device.
- **Single server model**: Zoo is the only server. No separate metadata
  service, no coordination with external systems.
- **S3-compatible storage**: All object storage goes through S3 API
  (MinIO, Backblaze B2, Wasabi, AWS S3, etc.).

---

## 4. Crate Layout

```
crates/
└── zoo/
    ├── Cargo.toml                    # Workspace member
    ├── src/
    │   ├── lib.rs
    │   ├── types.rs                  # All protocol types, state machine enums
    │   ├── state.rs                  # Upload state machine, transitions, validation
    │   ├── db/
    │   │   ├── mod.rs
    │   │   ├── models.rs             # DB row structs
    │   │   ├── devices.rs            # Device CRUD
    │   │   ├── uploads.rs            # Upload + parts CRUD
    │   │   └── files.rs              # File metadata CRUD
    │   ├── s3/
    │   │   ├── mod.rs
    │   │   ├── presigner.rs          # Presigned PUT/GET URL generation
    │   │   └── client.rs             # S3 client wrapper (HeadObject, ListParts, etc.)
    │   ├── sse/
    │   │   ├── mod.rs
    │   │   ├── hub.rs                # Per-user broadcast channel
    │   │   └── events.rs             # Event types, serialization
    │   ├── workers/
    │   │   ├── mod.rs
    │   │   ├── stall_detector.rs     # Detects stalled uploads
    │   │   └── garbage_collector.rs  # Orphan cleanup
    │   ├── api/
    │   │   ├── mod.rs                # Axum router construction
    │   │   ├── devices.rs            # Device registration endpoints
    │   │   ├── uploads.rs            # Upload lifecycle endpoints
    │   │   ├── files.rs              # Download proxy endpoint
    │   │   └── events.rs             # SSE endpoint
    │   ├── auth/
    │   │   ├── mod.rs
    │   │   └── tokens.rs             # Download token signing/verification
    │   └── config.rs                 # Environment/CLI configuration
    │
    ├── migrations/
    │   ├── 001_create_devices.sql
    │   ├── 002_create_uploads.sql
    │   ├── 003_create_upload_parts.sql
    │   └── 004_create_files.sql
    │
    └── bin/
        └── zoo-server.rs             # Binary entrypoint

crates/zoo-client/                   # Shared client SDK (no I/O)
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── types.rs                     # Re-exported protocol types
    ├── orchestrator.rs              # State machine event loop
    └── s3_uploader.rs               # S3 PUT part logic

crates/zoo-wasm/                     # WASM target for web clients
├── Cargo.toml
└── src/
    └── lib.rs                       # wasm-bindgen exports

docs/zoo/
├── ZOO.md                           # This file
└── dataflow.md                      # Sequence diagrams
```

---

## 5. Data Model

### 5.1 Table: `devices`

A device is registered once per install. The user chooses a human-readable
name. The SSE token authenticates the device's event stream.

```sql
CREATE TABLE devices (
    device_id   UUID PRIMARY KEY,
    user_id     TEXT NOT NULL,
    name        TEXT NOT NULL,        -- user-chosen, e.g. "Pixel 7"
    platform    TEXT NOT NULL,        -- android | ios | web | desktop
    sse_token   TEXT NOT NULL UNIQUE, -- bearer token for SSE auth
    push_token  TEXT,                 -- FCM / APNs token for push notifications
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ           -- tombstone on device removal
);

CREATE UNIQUE INDEX idx_devices_user_name ON devices(user_id, name);
CREATE INDEX idx_devices_sse_token ON devices(sse_token);
```

### 5.2 Table: `uploads`

Tracks every upload through its entire lifecycle. This is the system of record
for orchestration state. Records are cleaned up after successful registration.

```sql
CREATE TABLE uploads (
    upload_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          TEXT NOT NULL,
    device_id        UUID NOT NULL REFERENCES devices(device_id),
    status           TEXT NOT NULL DEFAULT 'pending',
    file_hash        TEXT NOT NULL,         -- SHA-256 of plaintext
    file_size        BIGINT NOT NULL,       -- plaintext bytes
    mime_type        TEXT,
    part_size        INT NOT NULL,          -- encrypted bytes per part
    part_count       SMALLINT NOT NULL,
    parts_bitmask    BYTEA NOT NULL DEFAULT '', -- variable-length bitset
    object_key       TEXT,                  -- S3 object key
    upload_id_s3     TEXT,                  -- S3 multipart upload ID
    complete_url     TEXT,                  -- presigned CompleteMultipartUpload URL
    urls_expire_at   TIMESTAMPTZ,           -- when current presigned URLs expire
    encrypting_at    TIMESTAMPTZ,
    uploading_at     TIMESTAMPTZ,
    last_heartbeat_at TIMESTAMPTZ,
    stalled_at       TIMESTAMPTZ,
    error_reason     TEXT,
    metadata         JSONB,                 -- client-provided encrypted metadata
    payload          JSONB,                 -- registration payload (encrypted keys etc.)
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMPTZ NOT NULL,  -- GC deadline
    done_at          TIMESTAMPTZ            -- when status reached DONE
);

CREATE INDEX idx_uploads_user_status ON uploads(user_id, status);
CREATE INDEX idx_uploads_user_file_hash ON uploads(user_id, file_hash);
CREATE INDEX idx_uploads_heartbeat ON uploads(last_heartbeat_at)
    WHERE status = 'uploading';
CREATE INDEX idx_uploads_expires ON uploads(expires_at)
    WHERE status NOT IN ('done', 'failed');
```

### 5.3 Table: `upload_parts`

Pre-populated at presign time. The server defines what parts exist; the client
only reports progress. Enables bitmask vs ListParts reconciliation.

```sql
CREATE TABLE upload_parts (
    upload_id    UUID NOT NULL REFERENCES uploads(upload_id) ON DELETE CASCADE,
    part_number  SMALLINT NOT NULL,        -- 0-indexed
    part_size    INT NOT NULL,             -- encrypted bytes
    part_md5     TEXT NOT NULL,            -- base64 MD5 of encrypted part
    etag         TEXT,                     -- S3 ETag, populated on success
    status       TEXT NOT NULL DEFAULT 'pending',  -- pending | uploaded
    PRIMARY KEY (upload_id, part_number)
);
```

### 5.4 Table: `files`

Long-term file metadata. Inserted at registration (S3_COMPLETED →
REGISTERING). Read on download to verify existence and fetch object key.
Persisted indefinitely.

```sql
CREATE TABLE files (
    file_id             BIGSERIAL PRIMARY KEY,
    user_id             TEXT NOT NULL,
    upload_id           UUID NOT NULL REFERENCES uploads(upload_id),
    collection_id       TEXT NOT NULL,
    object_key          TEXT NOT NULL,
    file_size           BIGINT NOT NULL,
    mime_type           TEXT NOT NULL,
    encrypted_key       TEXT NOT NULL,       -- fileKey encrypted with collectionKey
    key_decryption_nonce TEXT NOT NULL,
    file_decryption_header TEXT NOT NULL,
    thumb_decryption_header TEXT,
    encrypted_metadata  TEXT NOT NULL,
    encrypted_thumbnail TEXT,
    thumbnail_size      INT,
    content_hash        TEXT NOT NULL,       -- SHA-256 of plaintext (dedup)
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at         TIMESTAMPTZ          -- tombstone for soft-delete
);

CREATE INDEX idx_files_user_id ON files(user_id);
CREATE INDEX idx_files_collection ON files(user_id, collection_id);
CREATE INDEX idx_files_content_hash ON files(user_id, content_hash);
CREATE INDEX idx_files_object_key ON files(object_key);
```

---

## 6. Upload State Machine

```
                    ┌──────────┐
                    │ PENDING  │ ◄── POST /api/uploads
                    └────┬─────┘
                         │
                    ┌────▼──────┐
               ┌───►│ ENCRYPTING│ ◄── PATCH { status: "encrypting" }
               │    └────┬──────┘
               │         │
               │    ┌────▼───────┐
               │    │ UPLOADING  │ ◄── PATCH { status: "uploading", parts_bitmask }
               │    └──┬────┬────┘
               │       │    │
               │       │    └── PATCH { parts_bitmask } every N parts
               │       │        SSE→all: upload.progress
               │       │
               │    ┌────▼──────────┐
               │    │ S3_COMPLETED  │ ◄── PATCH { status: "s3_completed" }
               │    └────┬──────────┘
               │         │
               │    ┌────▼───────────┐
               │    │ REGISTERING    │ ◄── POST .../register (idempotent)
               │    └────┬───────────┘
               │         │
               │    ┌────▼────┐
               │    │  DONE   │
               │    └─────────┘
               │
               │    ┌─────────┐
               └────┤ STALLED │ ◄── no heartbeat for N seconds (server)
                    └────┬────┘
                         │
                    ┌────▼──────┐
                    │  FAILED   │ ◄── DELETE (user cancel) or GC expiry
                    └───────────┘
```

### 6.1 State Transitions

| From | To | Trigger | Source |
|------|----|---------|--------|
| PENDING | ENCRYPTING | Client: PATCH { status } | Client |
| ENCRYPTING | UPLOADING | Client: PATCH { status } | Client |
| UPLOADING | UPLOADING | Client: PATCH { parts_bitmask } | Client |
| UPLOADING | S3_COMPLETED | Client: PATCH { status: "s3_completed" } | Client |
| S3_COMPLETED | REGISTERING | Client: POST .../register | Client |
| REGISTERING | DONE | Internal: DB insert + SSE broadcast | Server |
| any | STALLED | Heartbeat timeout | Server worker |
| STALLED | ENCRYPTING | Client: PATCH { status: "resuming" } | Client |
| STALLED | FAILED | Client: DELETE (cancel) | Client |
| STALLED | FAILED | GC: expires_at reached | Server worker |
| UPLOADING | FAILED | S3 error: multipart aborted | Client |

The server never transitions state unilaterally except for STALLED → FAILED.
All forward progress (PENDING → DONE) is client-driven.

---

## 7. API Surface

All endpoints require authentication via an opaque session token. Zoo stores a
SHA-256 hash of each token in the database, mapping it to a user_id. On each
request Zoo hashes the provided token and looks up the user_id.

### 7.1 Device Management

```
POST /api/devices
  Auth: session token
  Body: { name: string, platform: string, push_token?: string }
  Response: { device_id: UUID, sse_token: string }
  Errors: 409 (name already in use for this user)

PATCH /api/devices/me
  Auth: session token
  Body: { name?: string, push_token?: string }
  Response: 200 OK

DELETE /api/devices/me
  Auth: session token
  Response: 200 OK
  Side effects: tombstone device, cancel pending uploads from this device,
                invalidate SSE token
```

### 7.2 Upload Lifecycle

```
POST /api/uploads
  Auth: session token
  Body: { file_hash: string, file_size: u64, mime_type: string,
          collection_id: string, metadata: object }
  Response: { upload_id: UUID, status: "pending", device_name: string }
  Errors: 409 (active upload exists for this file_hash + collection_id)

PATCH /api/uploads/{upload_id}
  Auth: session token
  Body: { status: string, parts_bitmask?: string (base64) }
  Response: current UploadState object
  SSE: broadcast upload.progress to all user's connected devices
  Errors: 400 (invalid state transition), 404, 409 (status conflict)

POST /api/uploads/{upload_id}/presign
  Auth: session token
  Body: { part_size: u32, part_md5s: string[] }
  Response: { object_key: string, part_urls: string[],
              complete_url: string, urls_expire_at: string }
  Precondition: status == ENCRYPTING or UPLOADING
  Side effects: inserts part rows, CreateMultipartUpload on S3
  Errors: 400 (part_count > 10000, part_size outside bounds)

POST /api/uploads/{upload_id}/presign-refresh
  Auth: session token
  Body: {}
  Response: { part_urls: string[], complete_url: string, urls_expire_at: string }
  Precondition: status == UPLOADING
  Behavior: refreshes URLs for parts still marked pending in DB

POST /api/uploads/{upload_id}/register
  Auth: session token
  Body: { encrypted_key, key_decryption_nonce, file_decryption_header,
          thumb_decryption_header?, encrypted_metadata, encrypted_thumbnail?,
          collection_id, file_size, thumbnail_size?, mime_type }
  Response: { file_id: i64 }
  Idempotent: retrying with the same upload_id returns the same file_id
  Side effects: HeadObject verification on S3, insert files row,
                delete uploads + upload_parts rows
  Errors: 400 (size mismatch vs S3), 404

DELETE /api/uploads/{upload_id}
  Auth: session token
  Response: 200 OK
  Side effects: AbortMultipartUpload on S3 (if upload_id_s3 set),
                DeleteObject on S3 (if no parts completed),
                mark uploads status = FAILED
  Errors: 404
```

### 7.3 File Visibility & Download

```
GET /api/uploads?status=active|stalled|all
  Auth: session token
  Response: [ UploadSummary ]
  Returns uploads belonging to the authenticated user.
  Used to render the pending uploads queue / banner.

GET /api/uploads/{upload_id}
  Auth: session token
  Response: full UploadState including parts list

GET /api/files/{file_id}/download
  Auth: session token
  Response: HTTP 302 redirect to presigned S3 GET URL
  Side effects: validates file ownership via files
  Alternative: Zoo proxies the GET and streams the encrypted bytes
  (proxy mode vs redirect mode is configurable)

GET /api/files/{file_id}/thumbnail
  Auth: session token
  Response: HTTP 302 redirect to presigned S3 GET URL for thumbnail object
```

### 7.4 SSE Event Stream

```
GET /api/events
  Auth: Bearer <sse_token> (device-level token, not session token)
  Accept: text/event-stream
  Response: infinite text/event-stream

Event format:
  event: <event_type>
  data: <json_payload>

Event types:

upload.progress
  data: { upload_id, status, parts_bitmask, part_count, device_name }

upload.completed
  data: { upload_id, device_name }

upload.done
  data: { upload_id, file_id, device_name }

upload.stalled
  data: { upload_id, parts_bitmask, part_count, device_name, stalled_at }

upload.failed
  data: { upload_id, reason, device_name }

device.connected
  data: { device_id, device_name }

device.disconnected
  data: { device_id, device_name }

heartbeat
  data: { timestamp }

KEEPALIVE: server sends "heartbeat" every 15 seconds
```

The SSE stream is per-user. Any event affecting the user is broadcast to all
their connected devices. The device_name in each event identifies which device
triggered it.

---

## 8. SSE Hub Implementation

### 8.1 In-Memory Hub

```rust
pub struct SseHub {
    // user_id -> broadcast channel
    channels: RwLock<HashMap<String, broadcast::Sender<SseEvent>>>,
}

impl SseHub {
    pub fn subscribe(&self, user_id: &str) -> broadcast::Receiver<SseEvent> {
        let mut channels = self.channels.write();
        let sender = channels.entry(user_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0);
        sender.subscribe()
    }

    pub fn broadcast(&self, user_id: &str, event: SseEvent) {
        if let Some(sender) = self.channels.read().get(user_id) {
            let _ = sender.send(event); // ignore recv errors
        }
    }
}
```

### 8.2 SSE Endpoint (Axum)

```rust
async fn events_handler(
    auth: SseAuth,           // extracts device_id, user_id from sse_token
    State(hub): State<SseHub>,
) -> Response {
    let mut rx = hub.subscribe(&auth.user_id);

    let stream = async_stream::stream! {
        // Send current pending/stalled state on connect
        let pending = get_pending_uploads(&auth.user_id).await;
        if !pending.is_empty() {
            yield Ok::<_, Infallible>(Event::default()
                .event("upload.pending")
                .json_data(pending).unwrap());
        }

        // Stream live events
        while let Some(event) = rx.recv().await {
            let sse_event = match event {
                SseEvent::Heartbeat { .. } => {
                    Event::default().event("heartbeat").json_data(event).unwrap()
                }
                other => {
                    let ty = other.event_type();
                    Event::default().event(ty).json_data(other).unwrap()
                }
            };
            yield Ok(sse_event);
        }
    };

    // Also spawn a heartbeat ticker
    let heartbeat_stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            yield Ok::<_, Infallible>(
                Event::default().event("heartbeat")
                    .json_data(serde_json::json!({"timestamp": Utc::now()})).unwrap()
            );
        }
    };

    Sse::new(stream::select(stream, heartbeat_stream))
        .keep_alive(KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text(": heartbeat\n"))
}
```

### 8.3 Horizontal Fan-Out (multi-instance)

For multiple Zoo replicas, the hub uses Postgres LISTEN/NOTIFY:

```rust
// On any event:
sql: NOTIFY events, '<json payload>';

// Listener in each replica:
sql: LISTEN events;

// On NOTIFY:
rx.recv() -> parse payload -> broadcast to local SseHub
```

This is zero-dependency — every Postgres driver supports LISTEN/NOTIFY.
No Redis, no NATS, no Kafka.

---

## 9. Stall Detection

### 9.1 Worker

```rust
// Runs every 15 seconds
SELECT upload_id, user_id, device_id, parts_bitmask, part_count
FROM uploads
WHERE status = 'uploading'
  AND last_heartbeat_at < NOW() - $1::interval  -- default: 90 seconds
FOR UPDATE SKIP LOCKED;

// For each result:
//   1. UPDATE uploads SET status = 'stalled', stalled_at = NOW()
//   2. hub.broadcast(user_id, SseEvent::UploadStalled { ... })
//   3. Send push notification to originating device (if push_token registered)
```

### 9.2 Sensitivity Configuration

Users can configure stall detection sensitivity per-device:

```
Device settings:
  Stall timeout: aggressive (30s) | normal (90s) | relaxed (5min) | none

Stored in devices as a JSONB settings field.
```

### 9.3 Client Guarantee

During UPLOADING state, the client sends a PATCH at least once every 30
seconds. After completing a batch of parts, the PATCH carries the updated
parts_bitmask. If no parts were completed in the last 30 seconds, the client
sends a PATCH with the same bitmask (pure heartbeat).

```

---

## 10. Resume Protocol

Resume is only possible from the same device that originated the upload. The
source file exists only on that device.

### 10.1 Client Reconnect

```
CLIENT                    ZOO                        S3
  │                         │                          │
  │  [app relaunch]         │                          │
  │                         │                          │
  ├─ SSE connect ──────────→│                          │
  │← upload.pending ────────│                          │
  │  [{upload_id: "abc",    │                          │
  │    status: "stalled",   │                          │
  │    parts_bitmask: 63,   │                          │
  │    part_count: 10}]     │                          │
  │                         │                          │
  ├─ PATCH /uploads/abc ───→│                          │
  │  { status: "resuming" } │                          │
  │                         │                          │
  ├─ GET /uploads/abc ─────→│                          │
  │← { parts_bitmask,       │                          │
  │    part_count,           │                          │
  │    object_key,           │                          │
  │    upload_id_s3 } ──────│                          │
  │                         │                          │
  ├─ ListParts ──────────────────────────────────────→│
  │← [{part: 0, etag: "a"},                           │
  │   {part: 1, etag: "b"}, ...] ────────────────────│
  │                         │                          │
  │  [reconcile:            │                          │
  │   part 0: Zoo=pending, S3=missing → upload new    │
  │   part 1: Zoo=uploaded, S3=etag "b" matches DB → skip
  │   part 2: Zoo=pending, S3=missing → re-encrypt + upload
  │   ...]                   │                          │
  │                         │                          │
  ├─ POST .../presign-refresh →│                      │
  │← { part_urls[2..9],     │                          │
  │    complete_url } ──────│                          │
  │                         │                          │
  │  [re-encrypt + upload   │                          │
  │   missing parts 2..9]   │                          │
  │                         │                          │
  ├─ PATCH ... + progress ─→│  SSE→all: progress       │
```

### 10.2 S3 as Ground Truth

On resume, Zoo's `parts_bitmask` and S3's `ListParts` are cross-referenced:

| Zoo bitmask | S3 ListParts | Action |
|-------------|--------------|--------|
| uploaded (1) | part exists, etag matches DB | Skip |
| uploaded (1) | part missing | Mark pending in Zoo, re-upload |
| uploaded (1) | part exists, etag differs | Mark pending in Zoo, re-upload |
| pending (0) | part missing | Re-upload |
| pending (0) | part exists | Unexpected — log + mark uploaded in Zoo |

If S3 returns `NoSuchUpload` (multipart was aborted via GC or manual action),
the client marks the upload as FAILED and starts fresh.

### 10.3 Web Resume

The web client persists only two values in localStorage:
- `upload_id` — the active upload token
- `file_handle` — a File System Access API handle (if available)

On page reload:
1. Read `upload_id` from localStorage
2. SSE connect + receive `upload.pending` event
3. If stalled, prompt user: "Resume upload of vacation.mp4? (60% done)"
4. If user still has the origin file tab open, resume via File API
5. If tab was closed or file gone, user can cancel the stalled upload

---

## 11. Download Proxy

### 11.1 Two Modes

**Mode A — Redirect (default)**: Zoo validates access, returns HTTP 302 to a
presigned S3 GET URL. Client follows the redirect and downloads directly from
S3. Minimal server load.

**Mode B — Proxy**: Zoo streams the file through itself. Useful when:
- S3 is not directly accessible from the client network
- You want to log or audit download access server-side
- You want to apply rate limiting per-user

Configurable per-deployment:

```rust
pub enum DownloadMode {
    Redirect { presigned_ttl: Duration },
    Proxy { max_concurrent: usize },
}
```

### 11.2 Access Control

Access is checked via `files` ownership:

```
GET /api/files/{file_id}/download
  Auth: session token
  
  1. SELECT FROM files WHERE file_id = $1
     → 404 if not found
     → 403 if file.archived_at IS NOT NULL
     → 403 if file.user_id != auth.user_id (owner-only in v1)
  
  2. For shared access (v2):
     → Check shares table for (file_id, auth.user_id)
     → If no share record, 403
  
  3a. Mode Redirect:
     → s3_client.GetObjectRequest(bucket, object_key).Presign(7 days)
     → return 302 { Location: presigned_url }
  
  3b. Mode Proxy:
     → s3_client.GetObject(bucket, object_key)
     → stream with Content-Type: application/octet-stream
```

### 11.3 Sharing Model (v2)

```sql
CREATE TABLE shares (
    file_id      BIGINT NOT NULL REFERENCES files(file_id),
    shared_with  TEXT NOT NULL,        -- user_id of the share recipient
    collection_id TEXT NOT NULL,       -- logical grouping for UI
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ,
    PRIMARY KEY (file_id, shared_with)
);
```

---

## 12. Orphan & Garbage Collection

### 12.1 Worker

```rust
// Runs every 5 minutes
SELECT upload_id, user_id, object_key, upload_id_s3
FROM uploads
WHERE status NOT IN ('done', 'failed')
  AND expires_at < NOW()
FOR UPDATE SKIP LOCKED;

// For each:
//   1. If upload_id_s3 is set → S3.AbortMultipartUpload
//   2. If object_key is set and no parts are in S3 → S3.DeleteObject
//   3. UPDATE uploads SET status = 'failed', error_reason = 'gc_expired'
//   4. hub.broadcast(user_id, SseEvent::UploadFailed { reason: "expired" })
```

### 12.2 Expiry Schedule

| State | Initial Expiry | Extended By |
|-------|---------------|-------------|
| PENDING | 1 hour | — |
| ENCRYPTING | 24 hours from last transition | — |
| UPLOADING | 24 hours from now | Heartbeat: reset to NOW() + 24h |
| STALLED | 7 days from stall | — |
| S3_COMPLETED | 1 hour | — |
| REGISTERING | 1 hour | — |
| DONE | Immediate cleanup | — |
| FAILED | 24 hours (tombstone for debugging) | — |

### 12.3 S3 Bucket Lifecycle Policy

As a safety net, configure an S3 lifecycle rule that deletes incomplete
multipart uploads older than 14 days and objects older than 30 days in a
designated "orphan" prefix:

```
S3 Lifecycle Rule:
  - Prefix: uploads/
  - AbortIncompleteMultipartUpload: 14 days
  - Expiration: 30 days (for orphaned objects)
```

This catches any object that Zoo's GC missed due to a bug or outage.

---

## 13. Client SDK

### 13.1 Core Crate: `zoo-client`

Platform-agnostic state machine and upload logic. No I/O — pure state
transitions and data transformation.

```rust
// zoo-client/src/lib.rs

pub struct ZooClient {
    config: ZooConfig,
    http: HttpClient,       // injected by platform
    storage: StateStore,    // injected by platform
    s3_client: S3Client,   // injected by platform
}

impl ZooClient {
    /// Full upload lifecycle
    pub async fn upload_file(&self, source: FileSource, metadata: Metadata)
        -> Result<FileId, ZooError>;

    /// Resume a stalled upload
    pub async fn resume_upload(&self, upload_id: Uuid, source: FileSource)
        -> Result<FileId, ZooError>;

    /// Cancel a pending/stalled upload
    pub async fn cancel_upload(&self, upload_id: Uuid) -> Result<(), ZooError>;

    /// List pending/stalled uploads
    pub async fn pending_uploads(&self) -> Result<Vec<UploadSummary>, ZooError>;

    /// SSE event stream
    pub async fn events(&self) -> Result<SseStream, ZooError>;

    /// Download file
    pub async fn download_file(&self, file_id: i64, destination: &Path)
        -> Result<(), ZooError>;
}
```

### 13.2 Orchestrator Loop

```rust
// Internal orchestrator — simplified pseudocode

async fn upload_impl(source: FileSource, config: UploadConfig) -> Result<FileId> {
    let mut upload = start_upload(config).await?;       // POST /api/uploads
    sse_connect(upload.user_id, upload.sse_token).await; // GET /api/events

    upload = patch_state(upload.id, "encrypting").await;

    let encrypted = encrypt_file(source, config.file_key).await;
    let part_md5s = compute_part_md5s(&encrypted);

    let presign = presign(upload.id, encrypted.part_size, &part_md5s).await;
    upload = patch_state(upload.id, "uploading", 0).await;

    let mut parts_mask = BitVec::new(presign.part_count);
    for (i, (part, url)) in enumerate(zip(encrypted.parts, presign.part_urls)) {
        let etag = s3_put(url, part).await?;
        parts_mask.set(i);
        if i % 5 == 0 || i == presign.part_count - 1 {
            upload = patch_heartbeat(upload.id, &parts_mask).await;
        }
    }

    s3_complete(presign.complete_url, &etags).await?;
    upload = patch_state(upload.id, "s3_completed").await;

    let file_id = register(upload.id, &registration_payload).await?;
    Ok(file_id)
}
```

### 13.3 WASM Target (`zoo-wasm`)

For web clients. Compiles to wasm-pack compatible output.

```rust
// zoo-wasm/src/lib.rs

#[wasm_bindgen]
pub struct ZooHandle { /* opaque handle to ZooClient */ }

#[wasm_bindgen]
impl ZooHandle {
    pub async fn create(config: JsValue) -> Result<ZooHandle, JsError>;
    pub async fn upload_file(&self, file: JsValue) -> Result<JsValue, JsError>;
    pub async fn pending_uploads(&self) -> Result<JsValue, JsError>;
    pub async fn cancel_upload(&self, upload_id: &str) -> Result<(), JsError>;
    pub fn on_event(&self, cb: &js_sys::Function);
    pub fn close(&self);
}
```

The JS side manages the SSE `EventSource` natively and bridges it to the WASM
module:

```typescript
// zoo-client.js (thin wrapper)
import init, { ZooHandle } from "./pkg/wasm.js";

export class Zoo {
  private handle: ZooHandle;

  async connect(endpoint: string, deviceName: string) {
    await init();
    this.handle = await ZooHandle.create({ endpoint, deviceName });

    // Native EventSource for SSE (not going through WASM)
    const sse = new EventSource(`${endpoint}/api/events`, {
      withCredentials: true,
    });
    sse.addEventListener("upload.progress", (e) => {
      this.handle.on_event(new Function("return " + e.data)());
    });
  }
}
```

### 13.4 Platform Abstraction

The client SDK requires three platform-provided implementations:

Phase 1 ships web (WASM) and desktop (Tauri). Mobile is deferred.

| Trait | Web (WASM) | Desktop (Tauri) |
|-------|------------|-----------------|
| `HttpClient` | `fetch` via `wasm-bindgen` | `reqwest` (native TLS) |
| `StateStore` | `localStorage` / `IndexedDB` via `wasm-bindgen` | Filesystem |
| `S3Client` | `fetch` for PUT | `reqwest` or `aws-sdk-rust` |

The zoo-client crate defines these as async traits. Each platform provides its
own implementation.

---

## 14. Dedup Protection

### 14.1 Active Upload Dedup

```sql
UNIQUE INDEX idx_uploads_active_dedup ON uploads(user_id, file_hash)
    WHERE status IN ('pending', 'encrypting', 'uploading');
```

If device A starts uploading `vacation.mp4` and device B tries the same,
device B gets a `409 Conflict` response with the existing `upload_id` and
a message: *"vacation.mp4 is already being uploaded from Pixel 7"*.

### 14.2 File-Level Dedup

On registration, before inserting into `files`:

```sql
SELECT file_id FROM files
WHERE user_id = $1 AND content_hash = $2 AND archived_at IS NULL;
```

If a file with the same content hash already exists, the new registration
is a no-op — it returns the existing `file_id`. The client can optionally
create a collection link without re-uploading.

---

## 15. Configuration

```rust
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
    pub stall_sensitivities: Vec<StallSensitivity>,
    pub presigned_ttl: Duration,              // default: 24h
    pub gc_interval: Duration,                // default: 5m
    pub max_file_size: u64,                   // default: 10GiB
    pub max_part_count: u16,                  // default: 10000
    pub default_part_size: u32,               // default: 20MiB
}
```

Environment variable mapping:

```
LISTEN_ADDR=
DATABASE_URL=
S3_ENDPOINT=
S3_REGION=
S3_BUCKET=
S3_ACCESS_KEY=
S3_SECRET_KEY=
SESSION_TTL_DAYS=30
DOWNLOAD_MODE=redirect|proxy
STALL_TIMEOUT_SECONDS=90
PRESIGNED_TTL_HOURS=24
GC_INTERVAL_SECONDS=300
MAX_FILE_SIZE=10737418240
DEFAULT_PART_SIZE=20971520
```

---

## 16. Failure Mode Reference

| Scenario | Detection | Recovery |
|----------|-----------|----------|
| Client closes tab mid-upload | SSE drops, 90s heartbeat timeout → STALLED | Reopen tab, SSE reconnects, server sends stall event, client offers resume |
| Mobile enters tunnel (no signal) | No PATCH for 90s → STALLED | Signal returns → PATCH + ListParts → resume from where it left off |
| Zoo restarts | All SSE connections drop | Clients reconnect SSE, Zoo sends current state from DB, client decides resume or cancel |
| Zoo permanently dies | Uploads stuck in DB | GC worker runs on restart (or manual admin recovery from S3) |
| S3 region temporarily unavailable | Client gets 503 on part PUT | Client retries with exponential backoff (up to 5 attempts), then FAILED |
| User switches device | Other device SSE receives stall event | Sees upload queue with device_name; can cancel from old device, start fresh from this device |
| User reinstalls app | Device re-registers (new device_id) | Stalled uploads tied to old device_id visible via GET /api/uploads; user sees orphan queue, can cancel |
| Concurrent upload (same file_hash) | UNIQUE index on active uploads | Second device gets 409 + existing upload_id + originating device_name |
| CompleteMultipartUpload succeeds but POST /register fails | Client retries POST /register | Idempotent — Zoo checks files for existing upload_id, returns same file_id |
| Client-reported bitmask vs S3 ListParts mismatch | Detected on resume | S3 is ground truth; reconcile bitmask, re-upload missing parts |
| Clock drift breaks presigned URL signature | Client gets 403 from S3 | Client requests POST .../presign-refresh for new URLs |
| S3 multipart already completed or aborted | Client gets error on part PUT | If aborted: mark FAILED, start fresh. If completed: proceed to S3_COMPLETED. |

---

## 17. API Error Schema

```json
{
  "error": {
    "code": "upload_already_exists",
    "message": "vacation.mp4 is already being uploaded from Pixel 7",
    "details": {
      "upload_id": "550e8400-e29b-41d4-a716-446655440000",
      "device_name": "Pixel 7",
      "status": "uploading",
      "progress": "60%"
    }
  }
}
```

Standard error codes:

| Code | HTTP | Meaning |
|------|------|---------|
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
| `internal_error` | 500 | Unexpected server error |

---

## 18. Security Model

### 18.1 Authentication

- Session tokens are opaque random tokens. Zoo stores a SHA-256 hash of each
  token in the database (mapping to user_id). Tokens are revoked by deleting
  the database row — instant, no key rotation.
- SSE tokens are opaque bearer tokens issued at device registration, scoped
  only to the events endpoint
- Download tokens are not used — every download goes through the session auth
  check against files ownership

### 18.2 Rate Limiting

| Endpoint | Limit | Window |
|----------|-------|--------|
| POST /api/uploads | 100 | per hour per user |
| POST .../presign | 50 | per upload |
| PATCH /api/uploads/{id} | 1 per 5 seconds | per upload |
| GET /api/files/{id}/download | 1000 | per hour per user |
| GET /api/events | 1 concurrent | per device |

### 18.3 Input Validation

- file_size: ≤ max_file_size (10 GiB)
- part_size: ≥ 5 MiB, ≤ 5 GiB
- part_count: ≥ 1, ≤ 10000
- part_md5s: must match part_count, each must decode to 16 bytes
- All string fields: length-limited, Unicode-safe, no null bytes

---

## 19. Non-Goals (v1)

| Feature | Rationale |
|---------|-----------|
| Cross-device partial resume | Source file exists only on originating device |
| Client-side SSE library | Web uses native EventSource; mobile uses platform HTTP streaming |
| P2P transfer | Requires WebRTC infrastructure, out of scope |
| General-purpose S3 proxy | Zoo's download path is specifically for authenticated file access |
| FCM/APNs push integration | Platform-specific; wired separately via existing push infrastructure |
| Multi-region S3 replication | Deployment-specific concern; configured outside Zoo |

---

## 20. Migration from Naive Upload

If a prior version of the system used presigned URLs without state tracking:

1. No data migration needed — existing objects in S3 are not affected
2. Existing files in DB are not affected
3. All new uploads go through Zoo; the old path is disabled at the client level
4. Clients can be updated independently — Zoo endpoints are additive

---

## 21. Open Questions (Pre-Implementation)

1. **Download token vs direct auth**: Should Zoo use signed download tokens
   (embedding `{ file_id, user_id, expiry }`) instead of querying `files` on
   every request? Tokens would reduce DB load for download but add complexity
   for sharing revocation.

2. **Push notification integration**: Should Zoo own push notification
   dispatch (FCM/APNs), or should it emit a webhook that a separate service
   handles? Currently leaning toward "emit webhook" to keep Zoo focused on
   the upload pipeline.

3. **Upload progress granularity**: Is per-part bitmask sufficient, or should
   Zoo support byte-level progress reporting for very large files where
   individual parts take minutes to upload?

4. **Concurrent uploads per user**: Should Zoo enforce a max number of
   concurrent uploads per user (e.g. 5) to prevent resource exhaustion?
