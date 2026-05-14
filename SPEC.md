# Rey Specification — Encryption, Sync & Thumbnails

## 1. Encryption Scheme

### 1.1 Key Hierarchy

```
Password → Argon2id → KeyEncryptionKey (KEK)
                             │
                     ┌───────┴───────┐
                     │               │
                     ▼               ▼
             XSalsa20-Poly1305    BLAKE2b-KDF
             encrypts MasterKey   derives VerificationKey
                     │               │
                     ▼               ▼
                MasterKey         Sent over TLS
                     │         (hashed with bcrypt
             ┌───────┼───────┐    on server side)
             │       │       │
             ▼       ▼       ▼
       Collection  File    Secret
         Keys      Keys     Key
                     │
                     ▼
             XChaCha20-Poly1305
             encrypts file data,
             metadata, thumbnails
```

### 1.2 Algorithms

| Purpose | Algorithm | Key Size | Nonce/Header |
|---|---|---|---|
| Key encryption | XSalsa20-Poly1305 | 256-bit | 24 bytes |
| File/thumbnail encryption | XChaCha20-Poly1305 | 256-bit | 24 bytes |
| Password hardening KDF | Argon2id v1.3 | — | 16-byte salt |
| Subkey derivation | BLAKE2b (keyed) | 256-bit | — |
| Sharing (asymmetric) | X25519 + XSalsa20-Poly1305 sealed box | 256-bit keypair | 32-byte ephemeral PK |
| Server-side password verification | bcrypt | — | — |

### 1.3 Authentication

Login uses a verification key derived from the KEK. The raw password
never leaves the device.

```
Client                                          Server
  │                                                │
  ├─ POST /api/auth/params ───────────────────────→│
  │   { email }                                    │
  │                                                │
  │← { kek_salt, mem_limit, ops_limit } ───────────│
  │   (same params for all emails, no enum)        │
  │                                                │
  ├─ Argon2id(password, kek_salt, mem, ops) → KEK  │
  ├─ BLAKE2b-KDF(KEK, "verification") → verify_key │
  ├─ SHA-256(verify_key) → verify_key_hash         │
  │                                                │
  ├─ POST /api/auth/login ────────────────────────→│
  │   { email, verify_key_hash }                   │
  │                                                │
  │← { session_token, key_attributes } ────────────│
  │   (opaque 32-byte random token)                │
  │                                                │
  ├─ XSalsa20-Poly1305 decrypt(encrypted_master_key,│
  │   key_nonce, KEK) → MasterKey                  │
  ├─ Store MasterKey in memory (secret-protected)  │
```

Server stores `bcrypt(verify_key_hash)` at registration. On login:
1. Looks up user by email
2. Compares `verify_key_hash` against stored bcrypt hash
3. If match: generate opaque session token, hash it, store in DB
4. Return `{ session_token, key_attributes }`

**Why this works:**
- Raw password never transmitted — only a derived hash
- Verify key is derived from KEK (which requires Argon2id)
- Server breach yields bcrypt(verify_key_hash) — attacker must
  reverse Argon2id to find the password (impossible)
- Server can reject wrong passwords before client does expensive
  Argon2id — but this reveals existence. To prevent enumeration,
  server always returns the same params regardless of email existence
  (client does Argon2id locally and only learns password correctness
  from login response)

**Why not SRP:** By 2026, TLS 1.3 is universal and enforced by all
major browsers. The MITM threat SRP protected against is no longer
realistic. The verification key pattern gives "good enough" zero-knowledge
proof without the complexity of SRP's custom math, server session state,
and user-enumeration protection hacks.

### 1.4 Argon2id Parameters

| Profile | Memory | Ops | Use case |
|---|---|---|---|
| Sensitive | 256 MiB | 4 | First unlock on desktop |
| Mobile | 128 MiB | 3 | First unlock on phone (avoids OOM) |
| Interactive | 64 MiB | 2 | Quick re-auth (same session) |

Adaptive fallback: if allocation fails, halve memory and double ops,
down to a 32 MiB floor. Starts at 256 MiB (not 1 GiB) to avoid
crashing on low-end mobile devices.

### 1.5 Key Generation

All keys are 256 bits, generated via `OsRng`:

| Key | Generated | Encrypted With | Stored Where |
|---|---|---|---|
| MasterKey | At signup | KEK (derived from password) | Server + Recovery |
| CollectionKey | Per album/folder | MasterKey | Server |
| FileKey | Per file | CollectionKey | Server (alongside file record) |
| SecretKey (X25519 private) | At signup | MasterKey | Server |
| RecoveryKey | At signup | MasterKey | Server |

### 1.6 Verification Key

A separate `verification_key` is derived from the KEK via BLAKE2b-KDF
(context `"verification"`, subkey ID 2). This is sent over TLS during
login and stored server-side as a bcrypt hash. It proves the client
knows the password without exposing the KEK or MasterKey.

If the verification key is compromised (e.g. server breach), the
attacker still cannot derive the KEK or MasterKey — only verify
password guesses offline. The KEK requires Argon2id + the original
password.

### 1.7 Cipher Agility

Every encrypted file record stores a `cipher` field identifying which algorithm
was used. This allows changing the cipher in the future without breaking
existing files.

| Cipher ID | Algorithm | Key Size | Nonce/Header | Status |
|---|---|---|---|---|
| `xchacha20-poly1305` | XChaCha20-Poly1305 | 256-bit | 24 bytes | Default |
| `xsalsa20-poly1305` | XSalsa20-Poly1305 | 256-bit | 24 bytes | Legacy (key wrapping only) |

Adding a new cipher (e.g., AES-256-GCM, AEGIS-128L) is a non-breaking change:
new files use the new cipher, old files keep their cipher ID. The client checks
the cipher field before decrypting.

### 1.8 Wire Formats

**Key encryption (SecretBox — XSalsa20-Poly1305):**
```
nonce (24 bytes) || MAC (16 bytes) || ciphertext
```

**File/thumbnail encryption (SecretStream — XChaCha20-Poly1305):**
```
header (24 bytes) || ciphertext
```

**Sharing (Sealed Box — X25519 + XSalsa20-Poly1305):**
```
ephemeral_pk (32 bytes) || MAC (16 bytes) || ciphertext
```

---

## 2. Local Sync Protocol

### 2.1 Overview

Clients maintain a local SQLite database that mirrors the server's file catalog. The sync is **incremental diff** — only changes since the last sync are transferred. The server never sees plaintext metadata.

### 2.2 API Endpoints

```
GET /api/sync/collections?since=<timestamp>
  Response: {
    collections: [{
      id: string,
      encrypted_name: string,
      name_decryption_nonce: string,
      encrypted_key: string,
      key_decryption_nonce: string,
      updation_time: number
    }],
    has_more: boolean,
    latest_updated_at: number
  }

GET /api/sync/files?collection_id=<id>&since=<timestamp>&limit=1000
  Response: {
    updated_files: [{
      id: number,
      collection_id: string,
      cipher: string,                        -- "xchacha20-poly1305" | "aegis-128l" | etc.
      encrypted_key: string,
      key_decryption_nonce: string,
      file_decryption_header: string,
      encrypted_metadata: string,
      thumb_decryption_header: string?,
      encrypted_thumbnail: string?,
      thumbnail_size: number?,
      file_size: number,
      mime_type: string,
      content_hash: string,
      updation_time: number
    }],
    deleted_file_ids: number[],
    has_more: boolean,
    latest_updated_at: number
  }

GET /api/sync/trash?since=<timestamp>
  Response: {
    deleted_files: [{ file_id, collection_id, updation_time }],
    has_more: boolean,
    latest_updated_at: number
  }
```

### 2.3 Version-Consistent Pagination

The server MUST NOT split a version group across pages. Implementation:

```
1. Request N+1 rows from DB
2. Group rows by updation_time
3. If last group is incomplete → discard it (will be on next page)
4. Return N rows with has_more = true/false
```

### 2.4 Client Sync Flow

```
sync_all():
  1. GET /api/sync/collections?since=last_collection_sync_time
  2. For each updated collection:
     a. Decrypt collection name with MasterKey
     b. Decrypt collection key with MasterKey
     c. Upsert into local collections table
  3. For each collection:
     a. Loop: GET /api/sync/files?collection_id=X&since=last_file_sync_time
     b. For each file:
        - Decrypt file key with collection key
        - Decrypt metadata with file key (XChaCha20-Poly1305)
        - Insert/update local files table
     c. Remove deleted_file_ids from local DB
     d. Save new cursor (latest_updated_at)
  4. GET /api/sync/trash
     a. Mark deleted files as archived in local DB
  5. Save all cursors to local storage
```

### 2.5 Local SQLite Schema

```sql
-- Collections (decrypted locally)
CREATE TABLE collections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,          -- decrypted
    encrypted_key   TEXT NOT NULL,          -- kept for re-encryption
    key_nonce       TEXT NOT NULL,
    updation_time   INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    archived_at     INTEGER
);

-- Files (decrypted locally)
CREATE TABLE files (
    id                  INTEGER PRIMARY KEY,
    collection_id       TEXT NOT NULL REFERENCES collections(id),
    cipher              TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    title               TEXT,               -- decrypted from metadata
    description         TEXT,               -- decrypted from metadata
    latitude            REAL,
    longitude           REAL,
    taken_at            INTEGER,
    file_size           INTEGER NOT NULL,
    mime_type           TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    encrypted_key       TEXT NOT NULL,
    key_nonce           TEXT NOT NULL,
    file_decryption_header TEXT NOT NULL,
    thumb_decryption_header TEXT,
    object_key          TEXT NOT NULL,       -- S3 key for full download
    thumbnail_path      TEXT,               -- local cache path
    updation_time       INTEGER NOT NULL,
    created_at          INTEGER NOT NULL,
    archived_at         INTEGER
);

CREATE INDEX idx_files_collection ON files(collection_id);
CREATE INDEX idx_files_title ON files(title COLLATE NOCASE);
CREATE INDEX idx_files_taken_at ON files(taken_at);
CREATE INDEX idx_files_archived ON files(archived_at) WHERE archived_at IS NULL;

-- Sync cursors (key-value)
CREATE TABLE sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Keys: "collections_since", "collection:{id}_since", "trash_since"
```

### 2.6 First Sync vs Incremental

| Scenario | Cursor | Behavior |
|---|---|---|
| First sync (new device) | `since=0` | Full download of all metadata + thumbnails |
| Normal sync | `since=last_sync` | Only files with `updation_time > cursor` |
| Re-login / clear data | Reset to `since=0` | Metadata fetched from scratch (thumbnails can reuse disk cache by file ID) |
| Offline | No sync | All operations against local DB. Queued changes saved for next sync. |

### 2.7 Offline Support

- All metadata is local — full search, browse, and grid view work offline
- Cached thumbnails render the grid
- Opening a full image requires network (unless previously cached for offline)
- Uploads are queued locally and sent when online

---

## 3. Thumbnail Pipeline

### 3.1 Generation

Thumbnails are generated **client-side** during upload. The server never generates or re-encodes thumbnails.

```
on_upload(file):
  1. Generate thumbnail:
     - Images: resize to max dimension 720px, JPEG quality 85
     - Videos: extract frame at 1s mark, resize to 720px
     - Max file size: 100 KB
  2. Encrypt thumbnail with fileKey (XChaCha20-Poly1305)
  3. Upload encrypted thumbnail to S3 (parallel with main file)
  4. Store decryption header in file record
```

### 3.2 Client Cache

```
Two-level cache:

Level 1 — In-Memory LRU:
  - Capacity: 500 thumbnails (~50 MB)
  - Key: file_id
  - Eviction: LRU, on memory pressure

Level 2 — Disk Cache:
  - Location: {app_cache_dir}/thumbnails/{file_id}
  - Format: raw decrypted JPEG bytes
  - Eviction: LRU, max 2 GB, cleaned on app idle
  - Persists across app restarts

On cache miss:
  1. GET /api/files/{file_id}/thumbnail → redirect to S3 presigned URL
  2. Download encrypted bytes
  3. Decrypt with fileKey (XChaCha20-Poly1305)
  4. Write to disk cache
  5. Retain in memory cache
  6. Render

All downloads are queue-deduplicated — only one in-flight request per file_id.
```

### 3.3 Thumbnail Endpoints

```
GET /api/files/{file_id}/thumbnail
  Auth: session token
  Response: 302 redirect to presigned S3 GET URL
  Precondition: file exists and belongs to user

  Mode A — Redirect (default):
    → Generate presigned S3 URL (TTL: 7 days)
    → Return 302 Location: <presigned_url>
    Client follows redirect and downloads directly from S3.

  Mode B — Proxy:
    → Zoo streams the encrypted bytes through itself
    → Client decrypts after receiving
```

### 3.4 Cache Invalidation

| Event | Action |
|---|---|
| File deleted/archived | Remove thumbnail from disk + memory cache |
| Thumbnail re-uploaded | Invalidate cache entry, re-download on next view |
| App cache cleared | All thumbnails re-downloaded on demand |
| Disk space low | OS may evict cache directory; handled as cache miss |

---

## 4. Search

Search is **100% client-side** against the local SQLite database. No search endpoints exist on the server.

```sql
-- Text search
SELECT * FROM files
WHERE (title LIKE '%query%' OR description LIKE '%query%')
  AND archived_at IS NULL
ORDER BY taken_at DESC
LIMIT 50;

-- Geographic search
SELECT * FROM files
WHERE latitude BETWEEN ? AND ?
  AND longitude BETWEEN ? AND ?
  AND archived_at IS NULL;

-- Date range
SELECT * FROM files
WHERE taken_at BETWEEN ? AND ?
  AND archived_at IS NULL
ORDER BY taken_at DESC;
```

### 4.1 ML-Powered Search (Future)

For semantic ("magic") search:
- On-device CLIP model generates 512-dimension embeddings per image
- Embeddings stored in local vector database (USearch)
- Text query → CLIP text encoder → vector → ANN search → ranked results
- All on-device, zero network

---

## 5. Security Model

### 5.1 What the Server Knows

| Data | Server visibility |
|---|---|
| File size | Yes (required for S3) |
| MIME type | Yes (required for CDN) |
| Content hash (SHA-256 plaintext) | Yes (required for dedup) |
| Encrypted blob bytes | Yes (stored in S3) |
| Encrypted metadata | Yes (stored in DB) |
| Encrypted thumbnail | Yes (stored in S3) |
| Cipher identifier | Yes (required to know algorithm) |
| File title, description, location, date | **No** (encrypted in metadata) |
| FileKey, CollectionKey, MasterKey | **No** (only encrypted form) |
| Thumbnail pixels | **No** (encrypted with fileKey) |
| Share link key | **No** (in URL fragment, never sent) |

**Content hash privacy note:** SHA-256 of the plaintext is stored for
deduplication. If two users upload the same file, the server learns they
have identical content. This is a known leak in all content-addressed
systems. Mitigating with keyed hashes or blind dedup is possible but
beyond v1 scope.

### 5.2 Local Database Security

The client stores **decrypted** metadata in a local SQLite database. This
is the most sensitive data on disk — file names, locations, dates, tags.
Protection:

```
Desktop: DB file encrypted with platform keychain (macOS Keychain /
         Windows DPAPI / Linux secret-service). Key bound to the
         device, not the user account.

Mobile:  DB protected by iOS Keychain / Android Keystore. Additionally
         encrypted with a key derived from device biometrics (if enrolled).

In-memory: Plaintext keys held in secret-protected memory
           (mprotect(PROT_NONE) on Linux, VirtualProtect on Windows).
           Never written to swap.
```

If the local device is compromised, all metadata is readable. This is
an accepted tradeoff: privacy during transit (server never sees it)
vs. security at rest (device must be trusted).

---

## 6. Crate Responsibilities (Client)

| Crate | Responsibility |
|---|---|---|
| `types` | Define encrypted blob wire formats, key structs, nonce types, all protocol types |
| `crypto` | All encryption/decryption, key derivation, key generation |
| `image` | Image decode/encode, resize, EXIF extraction |
| `metadata` | Encrypt/decrypt FileMetadata structs with fileKey |
| `thumbnail` | Generate thumbnails, encrypt/decrypt, manage disk + memory cache |
| `common` | Config loading, unified error types, tracing/logging setup |
| `local-db` | Local SQLite database of decrypted metadata, search queries |
| `sync` | Incremental diff sync from Zoo, cursor tracking, batch decrypt |
| `client-lib` | Tauri command layer, wires sync + local-db + thumbnail |
| `zoo-client` | Upload/download state machine, SSE listener, no crypto |
