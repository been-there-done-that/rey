# Crate Structure

## Core Principle

Every crate has exactly one reason to change. The dependency graph is a DAG —
lower layers never import higher layers. Cargo enforces this at compile time.

```
   PLATFORM BINDINGS         SERVER
   (Tauri, WASM)             (zoo)
          │                    │
          ▼                    ▼
   CLIENT ORCHESTRATION   SERVER LOGIC
   (sync, DB, cache,      (monolithic:
    upload orchestrator)    api, db, s3, sse,
          │                 auth, workers)
          │
          ▼
   CORE LIBRARIES              SHARED
   (crypto, image,             (types, common)
    metadata, thumbnail)
```

---

## 1. Crate Map

### Layer 0 — Foundation (no internal deps)

| Crate | Path | Dependencies | Ships in |
|---|---|---|---|
| `types` | `crates/types/` | `serde`, `serde_json` (no internal) | client + server |
| `common` | `crates/common/` | `types` | client + server |

### Layer 1 — Pure Libraries (depend only on Layer 0)

| Crate | Path | Depends On | Ships in |
|---|---|---|---|
| `crypto` | `crates/crypto/` | `types` | client only |
| `image` | `crates/image/` | `types` | client only |
| `metadata` | `crates/metadata/` | `types`, `crypto`, `image` | client only |
| `thumbnail` | `crates/thumbnail/` | `types`, `crypto`, `image` | client only |

### Layer 2 — Application Logic

| Crate | Path | Depends On | Ships in |
|---|---|---|---|
| `local-db` | `crates/local-db/` | `types`, `common` | client only |
| `sync` | `crates/sync/` | `types`, `crypto`, `metadata`, `thumbnail`, `local-db`, `common` | client only |
| `zoo-client` | `crates/zoo-client/` | `types` | client only |
| `zoo` | `crates/zoo/` | `types`, `common` | server only |

### Layer 3 — Platform Bindings

| Crate | Path | Depends On | Ships in |
|---|---|---|---|
| `client-lib` | `crates/client-lib/` | `sync`, `local-db`, `thumbnail`, `zoo-client` | desktop (Tauri) |
| `zoo-wasm` | `crates/zoo-wasm/` | `zoo-client` | web (WASM) |

---

## 2. Crate Breakdown

### 2.1 `types` — Shared Types

Pure data types with `serde` derives. Zero logic.

```
crates/types/
├── Cargo.toml            # deps: serde, serde_json only
└── src/
    ├── lib.rs
    ├── crypto.rs         # EncryptedData, KeyAttributes, Nonce, Salt, KeyBundle
    ├── file.rs           # FileMetadata, FileRecord, EncryptedFileRecord
    ├── collection.rs     # Collection, EncryptedCollection
    ├── sync.rs           # SyncRequest, SyncResponse, DiffResponse, Cursor
    ├── upload.rs         # UploadState, UploadStatus, PartRecord
    ├── device.rs         # DeviceInfo, DeviceRegistration
    ├── share.rs          # ShareRequest, ShareResponse
    ├── user.rs           # UserInfo, SessionToken, KeyAttributes
    └── error.rs          # ErrorCode, ErrorResponse
```

### 2.2 `crypto` — Pure Encryption

All cryptography. Zero I/O. Zero platform dependencies. `#![no_std]` compatible.

```
crates/crypto/
├── Cargo.toml            # deps: types, aead, xchacha20poly1305,
│                         #       xsalsa20poly1305, x25519-dalek, argon2,
│                         #       blake2b_simd, rand_core
└── src/
    ├── lib.rs
    ├── aead/
    │   ├── mod.rs        # re-export: encrypt() / decrypt() dispatch
    │   ├── secretbox.rs  # XSalsa20-Poly1305 (key wrapping)
    │   └── stream.rs     # XChaCha20-Poly1305 (file/thumbnail data)
    ├── kdf/
    │   ├── mod.rs
    │   ├── argon.rs      # Argon2id (password → KEK)
    │   └── blake2b.rs    # BLAKE2b-KDF (masterKey → subkeys)
    ├── key/
    │   ├── mod.rs
    │   ├── generate.rs   # OsRng key generation
    │   ├── encrypt.rs    # Encrypt a key with another key (SecretBox)
    │   └── decrypt.rs    # Decrypt a key (SecretBox open)
    ├── seal/
    │   ├── mod.rs
    │   ├── box_.rs       # X25519 sealed box (sharing)
    │   └── keypair.rs    # X25519 keypair generation
    └── util.rs           # Constant-time compare, base64/hex encoding
```

### 2.3 `image` — Image Decode/Encode

Pixel-level image operations. No crypto.

```
crates/image/
├── Cargo.toml            # deps: types, image (image-rs), exif
└── src/
    ├── lib.rs
    ├── decode.rs          # Read: path → DynamicImage (JPEG, PNG, WebP, HEIC)
    ├── encode.rs          # Write: DynamicImage → bytes (JPEG quality, PNG compression)
    ├── resize.rs          # Resize: max_dimension, aspect-ratio-preserving
    ├── exif.rs            # Extract EXIF: GPS, datetime, orientation, camera make/model
    └── orientation.rs     # Auto-rotate based on EXIF orientation tag
```

### 2.4 `metadata` — Encrypted Metadata

Encrypts and decrypts file metadata. Bridges `crypto` and the metadata format.

```
crates/metadata/
├── Cargo.toml            # deps: types, crypto, image (for EXIF types)
└── src/
    ├── lib.rs
    ├── encrypt.rs         # FileMetadata → crypto::stream::encrypt → EncryptedFileRecord
    ├── decrypt.rs         # EncryptedFileRecord → crypto::stream::decrypt → FileMetadata
    ├── structs.rs         # FileMetadata { title, description, latitude, longitude,
                           #               taken_at, device_make, device_model, tags }
    └── magic.rs           # "Public magic metadata" — encrypted fields exposed
                           # for server-side sorting (e.g. encrypted taken_at)
```

### 2.5 `thumbnail` — Thumbnail Pipeline

Generates, encrypts, caches, and serves thumbnails.

```
crates/thumbnail/
├── Cargo.toml            # deps: types, crypto, image, metadata
└── src/
    ├── lib.rs
    ├── generate.rs        # path/file/bytes → resized JPEG ≤720px, ≤100KB
    ├── encrypt.rs         # raw thumbnail bytes → crypto::stream::encrypt
    ├── decrypt.rs         # encrypted thumbnail → crypto::stream::decrypt
    ├── cache/
    │   ├── mod.rs
    │   ├── memory.rs      # LRU in-memory cache (lru crate)
    │   └── disk.rs        # Disk-backed cache ({cache_dir}/thumbnails/{file_id})
    ├── download.rs        # Fetch from S3 (via Zoo redirect), decrypt, cache, return path
    └── preview.rs         # Generate preview for grid: decrypted thumbnail or placeholder
```

### 2.6 `common` — Cross-Cutting

Shared utilities used by both client and server. No crypto.

```
crates/common/
├── Cargo.toml            # deps: types, tracing, anyhow, thiserror
└── src/
    ├── lib.rs
    ├── config.rs          # Env/file config loading
    ├── error.rs           # Unified error enum
    ├── telemetry.rs       # tracing init, log format, OpenTelemetry
    ├── time.rs            # Utc timestamp helpers
    └── result.rs          # Result<T, Error>
```

### 2.7 `local-db` — Client-Side SQLite

Manages the local SQLite database of decrypted metadata. Purely data access.

```
crates/local-db/
├── Cargo.toml            # deps: types, common, rusqlite (or sqlx)
└── src/
    ├── lib.rs
    ├── connection.rs      # Open/close/migrate SQLite DB
    ├── migrations/        # Embedded SQL migration files
    ├── collections.rs     # CRUD: collections table
    ├── files.rs           # CRUD: files table (search queries here)
    ├── sync_state.rs      # Read/write sync cursors
    └── search.rs          # Full-text search queries against decrypted fields
```

### 2.8 `sync` — Sync Orchestration

Pulls diffs from Zoo, decrypts, writes to local DB. Pulls thumbnails on demand.

```
crates/sync/
├── Cargo.toml            # deps: types, crypto, metadata,
│                         #       thumbnail, local-db, common
└── src/
    ├── lib.rs
    ├── pull.rs            # Full sync loop: collections → files → trash
    ├── diff.rs            # Fetch + process a single diff page
    ├── decrypt.rs         # Decrypt batch of EncryptedFileRecords → FileRecords
    ├── thumbnails.rs      # Trigger thumbnail download for new files
    └── cursor.rs          # Track/persist sync cursors per collection
```

### 2.9 `zoo` — Monolithic Server

All server logic in a single crate with internal modules. See [ZOO.md](./ZOO.md) for the full module breakdown and API surface.

```
crates/zoo/
├── Cargo.toml              # deps: types, common, axum, tokio, sqlx, aws-sdk-s3, tower
├── src/
│   ├── lib.rs              # Crate root, module declarations
│   ├── config.rs           # Environment/CLI configuration
│   ├── types.rs            # Server-specific protocol types (upload state, events, etc.)
│   ├── state.rs            # Upload state machine, transitions, validation
│   ├── db/                 # Database layer (devices, uploads, files, sessions)
│   ├── s3/                 # S3 client (presigner, multipart, HeadObject)
│   ├── sse/                # SSE hub, events, Postgres LISTEN/NOTIFY fan-out
│   ├── workers/            # Stall detector, garbage collector
│   ├── api/                # Axum router, middleware, route handlers
│   └── auth/               # Opaque session token management, login, rate limiting
├── bin/
│   └── zoo-server.rs       # Binary entrypoint
└── migrations/             # SQL migration files
```

Dependencies: `types`, `common`, `axum`, `tokio`, `sqlx` (postgres), `aws-sdk-s3`, `tower`, `serde`.

Changes to this crate touch server logic only. The single-crate boundary ensures server code never leaks into client builds.

### 2.10 `zoo-client` — Client SDK

Platform-agnostic upload/download state machine. No I/O — traits are injected
by platform bindings. No crypto — works with encrypted bytes only.

```
crates/zoo-client/
├── Cargo.toml            # deps: types, reqwest (trait only, no default features)
└── src/
    ├── lib.rs
    ├── orchestrator.rs    # Upload state machine event loop
    ├── upload.rs          # Multipart upload logic (presign → PUT parts → complete)
    ├── download.rs        # Download file from Zoo redirect/proxy
    ├── sse.rs             # SSE event stream client
    └── types.rs           # Re-exported protocol types from types
```

### 2.11 Platform Bindings

```
crates/zoo-wasm/
├── Cargo.toml            # deps: zoo-client, wasm-bindgen, js-sys
└── src/
    └── lib.rs             # #[wasm_bindgen] wrappers around zoo-client

crates/client-lib/
├── Cargo.toml            # deps: types, sync, local-db,
│                         #       thumbnail, zoo-client, tauri (optional)
└── src/
    ├── lib.rs             # Module registration
    ├── commands/
    │   ├── mod.rs
    │   └── ...            # auth, files, collections, sync, upload, thumbnails, device
    └── state.rs           # AppState: DB connection, sync state, cache handles
```

---

## 3. Dependency Graph (Visual)

```
                              PHASE 1 PLATFORM LAYER
                        ┌──────────────────────────────────┐
                        │                                  │
                        │  ┌──────────┐     ┌────────────┐ │
                        │  │ zoo-wasm │     │   Tauri    │ │
                        │  │ (web)    │     │ (desktop)  │ │
                        │  └─────┬────┘     └──────┬─────┘ │
                        │        │                 │       │
                        └────────┼─────────────────┼───────┘
                                 │                 │
                          ┌──────┘                 └──────────┐
                          │                                    │
                          ▼                                    ▼
               ┌─────────────────┐              ┌──────────────────────┐
               │   CLIENT SDK    │              │  CLIENT ORCHESTRA    │
               │                 │              │                      │
               │  ┌───────────┐  │              │  ┌────────────────┐  │
               │  │zoo-client │  │              │  │  client-lib    │  │
               │  │ (I/O-free)│  │              │  │  (Tauri cmds)  │  │
               │  └───────────┘  │              │  └────┬────┬──────┘  │
               └─────────────────┘              │       │    │         │
                                                │  ┌────┘    │         │
                                                │  │         │         │
                                                │  ▼         ▼         │
                                                │  ┌──┐  ┌─────────┐  │
                                                │  │L │  │  sync   │  │
                                                │  │O │  └────┬────┘  │
                                                │  │C │       │       │
                                                │  │A │  ┌────▼────┐  │
                                                │  │L │  │metadata │  │
                                                │  │  │  └────┬────┘  │
                                                │  │D │       │       │
                                                │  │B │  ┌────▼────┐  │
                                                │  │  │  │ crypto  │  │
                                                │  │  │  │ + WASM  │  │
                                                │  └──┘  └─────────┘  │
                                                └──────────────────────┘

                               SERVER LAYER
                         ┌─────────────────────────────────┐
                         │              zoo                 │
                         │  ┌──────┐ ┌──────┐ ┌─────────┐  │
                         │  │ api  │ │ db   │ │ workers │  │
                         │  ├──────┤ ├──────┤ ├─────────┤  │
                         │  │ s3   │ │ sse  │ │ auth    │  │
                         │  └──────┘ └──────┘ └─────────┘  │
                         │  (internal modules, one crate)   │
                         └─────────────────────────────────┘

                         CORE LAYER (shared, no server deps)
                        ┌──────────────────────────────┐
                        │  ┌────────┐  ┌───────────┐   │
                        │  │ crypto │  │  image    │   │
                        │  │(no_std)│  │(image-rs) │   │
                        │  └───┬────┘  └─────┬─────┘   │
                        │      │              │         │
                        │  ┌───▼────┐  ┌──────▼──────┐ │
                        │  │metadata│  │  thumbnail  │ │
                        │  └───┬────┘  └──────┬──────┘ │
                        └──────┼──────────────┼────────┘
                               │              │
                               └──────┬───────┘
                                      │
                              ┌───────▼───────┐
                              │    types      │
                              │   (serde)     │
                              └───────┬───────┘
                                      │
                              ┌───────▼───────┐
                              │    common     │
                              │ (config,      │
                              │  error,       │
                              │  telemetry)   │
                              └───────────────┘
```

### Layer Ownership Summary

| Layer | Contains | Compile-time guarantee |
|---|---|---|
| **Layer 0 — Foundation** | `types`, `common` | No internal deps between them. Used by everything. |
| **Layer 1 — Core Libraries** | `crypto`, `image`, `metadata`, `thumbnail` | Zero framework coupling. No HTTP. No DB. No Tauri. Pure libraries. |
| **Layer 2 — Application Logic** | `sync`, `local-db`, `zoo-client`, `zoo` | Client and server separated at compile time. `zoo` never imports client crates. |
| **Layer 3 — Platform Bindings** | `client-lib`, `zoo-wasm` | No business logic. Only platform wrappers. |

### Import Rules

| Crate | Can import | Cannot import |
|---|---|---|
| `types` | `serde`, `serde_json` | any internal crate, any I/O library |
| `common` | `types`, std | any other internal crate |
| `crypto` | `types` | `image`, `sync`, any server crate, HTTP, file I/O |
| `image` | `types` | `crypto`, `sync`, any server crate, HTTP |
| `metadata` | `types`, `crypto`, `image` | `sync`, any server crate, HTTP |
| `thumbnail` | `types`, `crypto`, `image`, `metadata` | `sync`, any server crate, HTTP |
| `local-db` | `types`, `common`, `rusqlite` | `crypto`, any server crate |
| `sync` | `types`, `crypto`, `metadata`, `thumbnail`, `local-db`, `common` | any server crate, Tauri, platform-specific |
| `client-lib` | `sync`, `local-db`, `thumbnail`, `zoo-client` | any zoo server crate |
| `zoo-client` | `types` | `crypto`, any crypto/image crate |
| `zoo` | `types`, `common`, `axum`, `tokio`, `sqlx`, `aws-sdk-s3`, `tower` | `crypto`, `image`, any client crate |
| `zoo-wasm` | `zoo-client`, `wasm-bindgen` | any crypto/image crate |

---

## 4. What Lives Where

| Responsibility | Crate | Rationale |
|---|---|---|
| AES/XChaCha/Argon2 | `crypto` | Single audit surface. `no_std` compatible. |
| Key hierarchy (masterKey → collectionKey → fileKey) | `crypto::key` | Pure key derivation, no I/O. |
| Encrypt/decrypt file metadata | `metadata` | Bridges crypto output ↔ typed metadata structs. |
| Image resize, EXIF extraction | `image` | Image format churn isolated from crypto. |
| Thumbnail generation | `thumbnail::generate` | Needs `image` for resize. |
| Thumbnail encryption/decryption | `thumbnail::encrypt` / `decrypt` | Same as file encryption (XChaCha20 with fileKey). |
| Thumbnail disk/memory cache | `thumbnail::cache` | Co-located with thumbnail logic. |
| Local SQLite DB of decrypted metadata | `local-db` | Separate from sync — usable for tests without network. |
| Full-text search | `local-db::search` | SQLite FTS5 against local decrypted data. |
| Sync orchestration (pull diff) | `sync::pull` | Coordinates HTTP calls + decryption + DB writes. |
| Upload state machine (client) | `zoo-client::orchestrator` | No crypto. Works with encrypted bytes only. |
| Multipart S3 upload | `zoo-client::upload` | PUT parts, complete, abort. |
| Server DB queries, S3, SSE, auth, workers, API, sharing | `zoo` (internal modules) | All server logic co-located in a single crate. |
| WASM bindings | `zoo-wasm` | `#[wasm_bindgen]` wrappers. |
| Tauri commands | `client-lib::commands` | Thin wrappers, delegates to `sync` / `local-db`. |

---

## 5. File Tree

```
├── Cargo.toml                          # Virtual workspace
├── Cargo.lock
│
├── crates/
│   ├── types/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, crypto.rs, file.rs, collection.rs, sync.rs, upload.rs, ...)
│   │
│   ├── common/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, config.rs, error.rs, telemetry.rs, ...)
│   │
│   ├── crypto/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, aead/, kdf/, key/, seal/)
│   │
│   ├── image/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, decode.rs, encode.rs, resize.rs, exif.rs, ...)
│   │
│   ├── metadata/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, encrypt.rs, decrypt.rs, structs.rs, ...)
│   │
│   ├── thumbnail/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, generate.rs, encrypt.rs, decrypt.rs, cache/, ...)
│   │
│   ├── local-db/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, connection.rs, collections.rs, files.rs, search.rs, ...)
│   │
│   ├── sync/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, pull.rs, diff.rs, decrypt.rs, thumbnails.rs, cursor.rs)
│   │
│   ├── client-lib/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, commands/{auth, files, collections, sync, upload, thumbnails}, state.rs)
│   │
│   ├── zoo-client/
│   │   ├── Cargo.toml
│   │   └── src/ (lib.rs, orchestrator.rs, upload.rs, download.rs, sse.rs, types.rs)
│   │
│   ├── zoo/
│   │   ├── Cargo.toml
│   │   ├── src/ (lib.rs, config.rs, types.rs, state.rs,
│   │   │         db/, s3/, sse/, workers/, api/, auth/)
│   │   ├── bin/zoo-server.rs
│   │   └── migrations/ (*.sql files)
│   │
│   ├── zoo-wasm/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
    │
    ├── apps/
    │   ├── desktop/           # Tauri app (Phase 1)
│   │   ├── src/           # Frontend (React/Svelte)
│   │   ├── src-tauri/     # Thin, invokes client-lib commands
│   │   └── package.json
│   │
│   └── web/               # Optional Next.js web app
│       ├── src/
│       └── package.json
│
├── docs/
│   ├── ARCHITECTURE.md    # High-level architecture and platform decisions
│   ├── SPEC.md            # Encryption, sync, thumbnail specifications
│   ├── STRUCTURE.md       # This file — crate boundaries and responsibilities
│   └── ZOO.md             # Upload/download service specification
│
├── tmp/                   # Reference codebases for research
│   ├── ente/
│   └── seafile/
│
├── Makefile
└── CLAUDE.md
```

---

## 6. Compile-Time Guarantees

| Violation | How Cargo prevents it |
|---|---|---|
| Server accidentally links crypto | `zoo/Cargo.toml` does not list `crypto` |
| Client accidentally links axum | `client-lib/Cargo.toml` does not list `axum` |
| `image` depends on `crypto` | Only `types` is listed in `image/Cargo.toml` |
| `zoo-wasm` depends on `crypto` | Only `zoo-client` is listed in `zoo-wasm/Cargo.toml` |
| `zoo` leaks client logic | `zoo/Cargo.toml` does not list `sync`, `local-db`, or `client-lib` |
| Platform code leaks into core | Core crates have `#![no_std]` (crypto) or no framework deps |

---

## 7. Build Order & Compilation Parallelism

```
Step 1 (2 in parallel):  types, common

Step 2 (4 in parallel):  crypto, image, zoo-client, zoo

Step 3 (2 in parallel):  metadata, thumbnail

Step 4 (2 in parallel):  local-db, sync

Step 5 (2 in parallel):  client-lib, zoo-wasm
```

### Change Impact

| Change to | Recompiles |
|---|---|
| `types` | Everything |
| `crypto` | `metadata`, `thumbnail`, `sync`, `client-lib` |
| `image` | `metadata`, `thumbnail`, `sync`, `client-lib` |
| `zoo` | Nothing outside server (self-contained) |
| `sync` | `client-lib` only |
| `client-lib` | Nothing downstream (terminal crate) |

---

## 8. Feature Flags

| Crate | Feature | Effect |
|---|---|---|
| `crypto` | `default` | Full std crypto |
| `crypto` | `no_std` | Disable std, for embedded targets |
| `zoo` | `s3` | AWS S3 storage backend (default) |
| `zoo` | `local-fs` | Local filesystem storage backend (dev/testing) |
| `client-lib` | `desktop` | Enable Tauri desktop commands |

---

## 9. Testing Strategy Per Crate

| Crate | Test approach | What to test |
|---|---|---|
| `types` | Unit | Serialization round-trips, field validation |
| `crypto` | Unit + vectors | Encrypt/decrypt round-trips, libsodium cross-vectors |
| `image` | Unit + fixtures | Resize dimensions, EXIF extraction, format detection |
| `metadata` | Unit | Metadata encrypt/decrypt round-trip, magic metadata |
| `thumbnail` | Unit + fixtures | Generate → encrypt → decrypt → match original |
| `common` | Unit | Config parsing, error formatting |
| `local-db` | Integration (tmp sqlite) | CRUD operations, search queries, migration |
| `sync` | Integration (mock HTTP) | Diff pagination, cursor tracking, decrypt batch |
| `zoo` | Integration (test DB, MinIO) | API routes, DB queries, state machine, stall detection, GC, SSE |
| `zoo-client` | Integration (mock server) | Upload flow, resume, cancel, SSE reconnect |
