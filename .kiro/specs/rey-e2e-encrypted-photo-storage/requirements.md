# Requirements Document

## Introduction

Rey is an end-to-end encrypted photo storage application. Encryption and decryption execute exclusively on the user's device; the server (Zoo) stores only ciphertext and coordinates access. No plaintext, no keys, and no decrypted metadata ever cross the network boundary.

The system is composed of:
- A Rust Cargo workspace with layered crates (`crypto`, `image`, `metadata`, `thumbnail`, `sync`, `local-db`, `zoo-client`, `zoo`, `client-lib`, `zoo-wasm`)
- A Tauri desktop application (Phase 1 primary target: macOS, Windows, Linux)
- A web application (Phase 1 secondary target: browser via WASM)
- The Zoo server — a monolithic Rust service that owns upload/download, file metadata, access control, real-time events, and S3-compatible object storage

This document covers seven functional areas: Authentication & Key Management, File Encryption, Sync, Thumbnail Pipeline, Upload Service (Zoo), Search, and Platform Support.

---

## Glossary

- **KEK (Key Encryption Key)**: A 256-bit key derived from the user's password via Argon2id. Never stored or transmitted.
- **MasterKey**: A 256-bit random key generated at signup, encrypted with the KEK, and stored on the server. The root of the key hierarchy.
- **CollectionKey**: A 256-bit random key generated per album/folder, encrypted with the MasterKey, and stored on the server.
- **FileKey**: A 256-bit random key generated per file, encrypted with the CollectionKey, and stored on the server alongside the file record.
- **VerificationKey**: A subkey derived from the KEK via BLAKE2b-KDF (context `"verification"`, subkey ID 2). Used to prove password knowledge without transmitting the KEK.
- **VerifyKeyHash**: `SHA-256(VerificationKey)`. Transmitted to the server during login; stored server-side as `bcrypt(VerifyKeyHash)`.
- **SecretKey**: An X25519 private key generated at signup, encrypted with the MasterKey, used for asymmetric sharing.
- **RecoveryKey**: A random key generated at signup, encrypted with the MasterKey, used for account recovery.
- **SessionToken**: An opaque 32-byte random token returned by the server on successful login. Authenticates all subsequent API requests.
- **SseToken**: A device-scoped bearer token used exclusively to authenticate the SSE event stream.
- **Zoo**: The monolithic Rust server that handles upload/download, file metadata, access control, SSE events, and S3 storage.
- **Zoo_Client**: The platform-agnostic Rust client SDK (`zoo-client` crate) that drives the upload/download state machine.
- **Sync_Engine**: The `sync` crate that pulls incremental diffs from Zoo, decrypts them, and writes to the local SQLite database.
- **Local_DB**: The client-side SQLite database (`local-db` crate) that stores decrypted file metadata for offline access and search.
- **Thumbnail_Pipeline**: The `thumbnail` crate that generates, encrypts, caches, and serves thumbnails.
- **Crypto_Module**: The `crypto` crate that implements all AEAD encryption, key derivation, and key management.
- **Upload**: A Zoo record tracking a single file's upload lifecycle from PENDING through DONE.
- **Collection**: A logical grouping of files (album/folder) with its own CollectionKey.
- **SSE Hub**: The in-memory per-user broadcast channel in Zoo that delivers real-time events to all connected devices.
- **Stall_Detector**: The Zoo background worker that marks uploads as STALLED when no heartbeat is received within the configured timeout.
- **GC_Worker**: The Zoo background worker that expires and cleans up orphaned uploads and S3 objects.
- **Parts_Bitmask**: A variable-length byte array (big-endian bit vector, bit N = part N) stored in the `uploads` table to track which S3 multipart parts have been uploaded.
- **EARS**: Easy Approach to Requirements Syntax — the pattern language used for all acceptance criteria.
- **XChaCha20-Poly1305**: The AEAD cipher used for file data, metadata, and thumbnail encryption.
- **XSalsa20-Poly1305**: The AEAD cipher used for key wrapping (SecretBox).
- **Argon2id**: The password-hardening KDF used to derive the KEK from the user's password.
- **BLAKE2b-KDF**: The subkey derivation function used to derive the VerificationKey and other subkeys from the MasterKey.

---

## Requirements

---

### Requirement 1: User Registration and Key Bootstrapping

**User Story:** As a new user, I want to create an account so that my encryption keys are generated on my device and my password never leaves it.

#### Acceptance Criteria

1. WHEN a user submits a registration request with an email and password, THE Crypto_Module SHALL derive a KEK by running Argon2id v1.3 with the Sensitive profile (256 MiB memory, 4 iterations, 16-byte random salt).
2. WHEN the KEK is derived, THE Crypto_Module SHALL generate a 256-bit MasterKey via `OsRng`.
3. WHEN the MasterKey is generated, THE Crypto_Module SHALL encrypt it with the KEK using XSalsa20-Poly1305 (SecretBox) and produce a 24-byte nonce alongside the ciphertext.
4. WHEN the MasterKey is generated, THE Crypto_Module SHALL derive a VerificationKey via BLAKE2b-KDF with context `"verification"` and subkey ID 2.
5. WHEN the VerificationKey is derived, THE Crypto_Module SHALL compute `SHA-256(VerificationKey)` to produce the VerifyKeyHash.
6. WHEN the VerificationKey is derived, THE Crypto_Module SHALL generate an X25519 keypair (SecretKey + PublicKey) via `OsRng`.
7. WHEN the X25519 keypair is generated, THE Crypto_Module SHALL encrypt the SecretKey with the MasterKey using XSalsa20-Poly1305.
8. WHEN the X25519 keypair is generated, THE Crypto_Module SHALL generate a 256-bit RecoveryKey via `OsRng` and encrypt it with the MasterKey using XSalsa20-Poly1305.
9. WHEN the client sends a registration request to Zoo, THE registration request SHALL contain the email, VerifyKeyHash, encrypted MasterKey, key nonce, encrypted SecretKey, encrypted RecoveryKey, and Argon2id parameters (salt, mem_limit, ops_limit).
10. WHEN Zoo receives a registration request, THE Zoo SHALL store `bcrypt(VerifyKeyHash)` and never store the VerifyKeyHash in plaintext.
11. IF a registration request arrives for an email that already exists, THEN THE Zoo SHALL return HTTP 409 Conflict.
12. THE Crypto_Module SHALL never transmit the password, KEK, MasterKey, CollectionKey, FileKey, or SecretKey in plaintext over any network connection.

---

### Requirement 2: Authentication and Session Management

**User Story:** As a registered user, I want to log in so that I can access my encrypted files without the server learning my password or keys.

#### Acceptance Criteria

1. WHEN a client requests login parameters via `POST /api/auth/params` with an email, THE Zoo SHALL return the same Argon2id parameters (kek_salt, mem_limit, ops_limit) regardless of whether the email exists, to prevent user enumeration.
2. WHEN the client receives Argon2id parameters, THE Crypto_Module SHALL derive the KEK locally using Argon2id v1.3 with the returned parameters.
3. WHEN the KEK is derived, THE Crypto_Module SHALL derive the VerificationKey via BLAKE2b-KDF and compute `SHA-256(VerificationKey)` to produce the VerifyKeyHash.
4. WHEN the client sends `POST /api/auth/login` with the email and VerifyKeyHash, THE Zoo SHALL compare the VerifyKeyHash against the stored `bcrypt(VerifyKeyHash)` using a constant-time comparison function to prevent timing-based enumeration attacks.
5. WHEN the VerifyKeyHash comparison succeeds, THE Zoo SHALL generate a cryptographically random 32-byte SessionToken, store its SHA-256 hash mapped to the user_id, and return the opaque SessionToken and key_attributes to the client.
6. WHEN the client receives the SessionToken and key_attributes, THE Crypto_Module SHALL decrypt the encrypted MasterKey using the KEK and XSalsa20-Poly1305, and retain the MasterKey in secret-protected memory.
7. WHEN the VerifyKeyHash comparison fails, THE Zoo SHALL return HTTP 401 Unauthorized without revealing whether the email exists.
8. WHILE a valid SessionToken is held by the client, THE Zoo SHALL authenticate all API requests by hashing the provided token and looking up the user_id in the sessions table.
9. IF a request arrives with an invalid or expired SessionToken, THEN THE Zoo SHALL return HTTP 401 Unauthorized.
10. WHEN a user logs out, THE Zoo SHALL invalidate the SessionToken by deleting its hash from the sessions table.
11. THE Crypto_Module SHALL hold the MasterKey in memory protected by `mprotect(PROT_NONE)` on Linux, `VirtualProtect` on Windows, and equivalent OS mechanisms on macOS, and SHALL never write the MasterKey to disk or swap.

---

### Requirement 3: Argon2id Adaptive Fallback

**User Story:** As a user on a low-memory device, I want the application to adapt its key derivation parameters so that it does not crash or fail due to memory constraints.

#### Acceptance Criteria

1. WHEN Argon2id key derivation is initiated with the Sensitive profile (256 MiB), THE Crypto_Module SHALL attempt allocation at the full memory limit.
2. IF the Argon2id memory allocation fails, THEN THE Crypto_Module SHALL halve the memory parameter and double the ops parameter, and retry.
3. WHILE the memory parameter remains above 32 MiB, THE Crypto_Module SHALL continue the halve-and-double retry loop on allocation failure.
4. IF the memory parameter reaches 32 MiB and allocation still fails, THEN THE Crypto_Module SHALL return an error indicating that key derivation cannot proceed on this device.
5. WHERE the Mobile profile is selected, THE Crypto_Module SHALL use 128 MiB memory and 3 iterations as the starting parameters before applying the adaptive fallback.
6. WHERE the Interactive profile is selected, THE Crypto_Module SHALL use 64 MiB memory and 2 iterations as the starting parameters before applying the adaptive fallback.

---

### Requirement 4: Collection and File Key Management

**User Story:** As a user, I want each album and file to have its own encryption key so that sharing one collection does not expose other collections or files.

#### Acceptance Criteria

1. WHEN a new Collection is created, THE Crypto_Module SHALL generate a 256-bit CollectionKey via `OsRng`.
2. WHEN a CollectionKey is generated, THE Crypto_Module SHALL encrypt it with the MasterKey using XSalsa20-Poly1305 and produce a 24-byte nonce.
3. WHEN a new file is added to a Collection, THE Crypto_Module SHALL generate a 256-bit FileKey via `OsRng`.
4. WHEN a FileKey is generated, THE Crypto_Module SHALL encrypt it with the CollectionKey using XSalsa20-Poly1305 and produce a 24-byte nonce.
5. THE Zoo SHALL store the encrypted CollectionKey, its nonce, the encrypted FileKey, and its nonce alongside the respective records, and SHALL never store any key in plaintext.
6. WHEN a client decrypts a file, THE Crypto_Module SHALL first decrypt the CollectionKey with the MasterKey, then decrypt the FileKey with the CollectionKey, then decrypt the file data with the FileKey.
7. THE Crypto_Module SHALL support cipher agility by storing a `cipher` field with every encrypted record, defaulting to `xchacha20-poly1305`, so that future cipher migrations are non-breaking.

---

### Requirement 5: File Encryption and Decryption

**User Story:** As a user, I want my photos and videos to be encrypted on my device before upload so that the server never has access to my plaintext content.

#### Acceptance Criteria

1. WHEN a file is prepared for upload, THE Crypto_Module SHALL encrypt the file data using XChaCha20-Poly1305 (SecretStream) with the FileKey, producing a 24-byte header and ciphertext.
2. WHEN a file is prepared for upload, THE Crypto_Module SHALL encrypt the file metadata (title, description, latitude, longitude, taken_at, device_make, device_model, tags) using XChaCha20-Poly1305 with the FileKey.
3. WHEN a file is prepared for upload, THE Crypto_Module SHALL encrypt the thumbnail using XChaCha20-Poly1305 with the FileKey, producing a separate 24-byte decryption header.
4. THE Crypto_Module SHALL produce wire-format output for file encryption as: `header (24 bytes) || ciphertext`.
5. THE Crypto_Module SHALL produce wire-format output for key encryption (SecretBox) as: `nonce (24 bytes) || MAC (16 bytes) || ciphertext`.
6. WHEN a file is downloaded and decrypted, THE Crypto_Module SHALL verify the Poly1305 MAC before returning any plaintext bytes, and SHALL return an error if verification fails.
7. IF decryption fails due to a MAC mismatch, corrupted data, or unsupported cipher identifier, THEN THE Crypto_Module SHALL return a typed error and SHALL NOT return partial plaintext.
8. FOR ALL valid FileKey and plaintext pairs, encrypting then decrypting SHALL produce a byte-for-byte identical result to the original plaintext (round-trip property).

---

### Requirement 6: Asymmetric Sharing

**User Story:** As a user, I want to share a collection with another user so that they can decrypt its contents without me exposing my MasterKey.

#### Acceptance Criteria

1. WHEN a user shares a Collection with a recipient, THE Crypto_Module SHALL encrypt the CollectionKey using an X25519 sealed box (ephemeral X25519 keypair + XSalsa20-Poly1305) with the recipient's PublicKey.
2. THE Crypto_Module SHALL produce wire-format output for sealed-box sharing as: `ephemeral_pk (32 bytes) || MAC (16 bytes) || ciphertext`.
3. WHEN a recipient receives a shared Collection, THE Crypto_Module SHALL decrypt the sealed box using the recipient's SecretKey to recover the CollectionKey.
4. THE Zoo SHALL store share records in the `shares` table linking file_id, shared_with (user_id), collection_id, created_at, and optional expires_at.
5. WHEN a share record has an expires_at in the past, THE Zoo SHALL deny download access for that share and return HTTP 403 Forbidden.


---

### Requirement 7: Incremental Sync — Collections

**User Story:** As a user with multiple devices, I want my collection list to stay in sync so that albums I create on one device appear on all others.

#### Acceptance Criteria

1. WHEN the Sync_Engine initiates a collection sync, THE Sync_Engine SHALL call `GET /api/sync/collections?since=<cursor>` where the cursor is the `latest_updated_at` value from the previous sync, or 0 for the first sync.
2. WHEN Zoo receives a collection sync request, THE Zoo SHALL return only collections with `updation_time > since`, paginated with a `has_more` flag and a `latest_updated_at` cursor.
3. WHEN Zoo paginates collection results, THE Zoo SHALL NOT split a group of records sharing the same `updation_time` across two pages; if the last group is incomplete, THE Zoo SHALL discard it and set `has_more = true`.
4. WHEN the Sync_Engine receives a collection page, THE Sync_Engine SHALL decrypt each collection name and CollectionKey using the MasterKey and upsert the result into the Local_DB `collections` table.
5. WHEN the Sync_Engine successfully processes a page, THE Sync_Engine SHALL persist the `latest_updated_at` cursor to the `sync_state` table under key `"collections_since"`.
6. WHILE `has_more` is true, THE Sync_Engine SHALL continue fetching subsequent pages before advancing the cursor.
7. IF the Sync_Engine encounters a decryption failure for a collection record, THEN THE Sync_Engine SHALL log the error, skip that record, and continue processing remaining records.

---

### Requirement 8: Incremental Sync — Files

**User Story:** As a user, I want new and updated files to appear on all my devices without re-downloading the entire catalog.

#### Acceptance Criteria

1. WHEN the Sync_Engine syncs files for a Collection, THE Sync_Engine SHALL call `GET /api/sync/files?collection_id=<id>&since=<cursor>&limit=1000` using the per-collection cursor stored under key `"collection:{id}_since"`.
2. WHEN Zoo returns a file sync response, THE response SHALL include `updated_files` (array of encrypted file records), `deleted_file_ids` (array of integer IDs), `has_more`, and `latest_updated_at`.
3. WHEN the Sync_Engine receives updated file records, THE Sync_Engine SHALL decrypt each FileKey with the CollectionKey, decrypt the file metadata with the FileKey using XChaCha20-Poly1305, and upsert the decrypted record into the Local_DB `files` table.
4. WHEN the Sync_Engine receives `deleted_file_ids`, THE Sync_Engine SHALL set `archived_at` on those records in the Local_DB `files` table.
5. WHEN the Sync_Engine successfully processes a file sync page, THE Sync_Engine SHALL persist the `latest_updated_at` cursor under key `"collection:{id}_since"`.
6. WHILE `has_more` is true for a collection, THE Sync_Engine SHALL continue fetching subsequent pages before advancing the cursor.
7. WHEN the Sync_Engine syncs the trash, THE Sync_Engine SHALL call `GET /api/sync/trash?since=<cursor>` and mark all returned `deleted_file_ids` as archived in the Local_DB.
8. IF the Sync_Engine is offline, THEN THE Sync_Engine SHALL serve all read operations from the Local_DB without attempting network calls.

---

### Requirement 9: Local SQLite Database

**User Story:** As a user, I want to browse and search my photos offline so that the app is fully functional without a network connection.

#### Acceptance Criteria

1. THE Local_DB SHALL maintain a `collections` table with columns: id (TEXT PK), name (TEXT, decrypted), encrypted_key (TEXT), key_nonce (TEXT), updation_time (INTEGER), created_at (INTEGER), archived_at (INTEGER nullable).
2. THE Local_DB SHALL maintain a `files` table with columns: id (INTEGER PK), collection_id (TEXT FK), cipher (TEXT), title (TEXT), description (TEXT), latitude (REAL), longitude (REAL), taken_at (INTEGER), file_size (INTEGER), mime_type (TEXT), content_hash (TEXT), encrypted_key (TEXT), key_nonce (TEXT), file_decryption_header (TEXT), thumb_decryption_header (TEXT nullable), object_key (TEXT), thumbnail_path (TEXT nullable), updation_time (INTEGER), created_at (INTEGER), archived_at (INTEGER nullable).
3. THE Local_DB SHALL maintain a `sync_state` table with columns: key (TEXT PK), value (TEXT) for storing sync cursors.
4. THE Local_DB SHALL create indexes on `files(collection_id)`, `files(taken_at)`, and a partial index on `files(archived_at) WHERE archived_at IS NULL`.
5. WHEN the Local_DB is first opened on a device, THE Local_DB SHALL run all pending SQL migrations in order before accepting any queries.
6. THE Local_DB SHALL encrypt the SQLite database file at rest using a key bound to the platform credential store (macOS Keychain, Windows DPAPI, Linux secret-service).
7. IF the platform credential store is unavailable, THEN THE Local_DB SHALL return an error and SHALL NOT open the database in an unencrypted state.

---

### Requirement 10: Thumbnail Generation

**User Story:** As a user, I want thumbnails generated automatically during upload so that the photo grid loads quickly without downloading full-resolution files.

#### Acceptance Criteria

1. WHEN a file is prepared for upload, THE Thumbnail_Pipeline SHALL generate a thumbnail by resizing the image to a maximum dimension of 720 pixels while preserving the aspect ratio.
2. WHEN generating a thumbnail from a JPEG source, THE Thumbnail_Pipeline SHALL encode the output as JPEG at quality 85.
3. WHEN generating a thumbnail from a video source, THE Thumbnail_Pipeline SHALL extract the frame at the 1-second mark and resize it to a maximum dimension of 720 pixels.
4. WHEN a generated thumbnail exceeds 100 KB, THE Thumbnail_Pipeline SHALL reduce JPEG quality iteratively until the output is at or below 100 KB.
5. WHEN a thumbnail is generated, THE Thumbnail_Pipeline SHALL apply EXIF orientation correction before encoding.
6. WHEN a thumbnail is generated, THE Thumbnail_Pipeline SHALL encrypt it using XChaCha20-Poly1305 with the FileKey, producing a 24-byte decryption header.
7. FOR ALL valid FileKey and thumbnail byte pairs, encrypting then decrypting the thumbnail SHALL produce a byte-for-byte identical result to the original thumbnail bytes (round-trip property).
8. IF thumbnail generation fails for a file (unsupported format, corrupt input), THEN THE Thumbnail_Pipeline SHALL log the error and proceed with the upload without a thumbnail.

---

### Requirement 11: Thumbnail Cache

**User Story:** As a user, I want thumbnails to load instantly when I scroll through my photo grid so that the experience is smooth even on slow connections.

#### Acceptance Criteria

1. THE Thumbnail_Pipeline SHALL maintain a two-level cache: a Level 1 in-memory LRU cache with a capacity of 500 thumbnails (approximately 50 MB) and a Level 2 disk cache at `{app_cache_dir}/thumbnails/{file_id}`.
2. WHEN a thumbnail is requested and present in the Level 1 memory cache, THE Thumbnail_Pipeline SHALL return it without disk or network access.
3. WHEN a thumbnail is requested and absent from the Level 1 cache but present in the Level 2 disk cache, THE Thumbnail_Pipeline SHALL load it from disk, insert it into the Level 1 cache, and return it.
4. WHEN a thumbnail is requested and absent from both cache levels, THE Thumbnail_Pipeline SHALL fetch the encrypted thumbnail from Zoo via `GET /api/files/{file_id}/thumbnail`, decrypt it with the FileKey, write the decrypted bytes to the Level 2 disk cache, insert into the Level 1 cache, and return it.
5. THE Thumbnail_Pipeline SHALL deduplicate in-flight download requests so that at most one network request is in flight per file_id at any time.
6. WHEN a file is deleted or archived, THE Thumbnail_Pipeline SHALL remove the corresponding entry from both the Level 1 and Level 2 caches.
7. WHEN the Level 2 disk cache exceeds 2 GB, THE Thumbnail_Pipeline SHALL evict the least-recently-used entries until the cache is below 2 GB.
8. IF the Level 2 disk cache entry is missing (evicted by the OS or cleared by the user), THEN THE Thumbnail_Pipeline SHALL treat it as a cache miss and re-download on the next request.


---

### Requirement 12: Upload Initiation and Device Registration

**User Story:** As a user, I want my device to be registered with the server so that uploads are tracked per device and I can see which device is uploading what.

#### Acceptance Criteria

1. WHEN a client installs the application for the first time, THE Zoo SHALL register the device via `POST /api/devices` with a user-chosen name, platform identifier (android | ios | web | desktop), and optional push token.
2. WHEN a device is registered, THE Zoo SHALL generate a UUID device_id and a unique SseToken, store them in the `devices` table, and return both to the client.
3. IF a device registration request uses a name already in use for that user, THEN THE Zoo SHALL return HTTP 409 Conflict.
4. WHEN a client initiates an upload via `POST /api/uploads`, THE Zoo SHALL create an upload record with status PENDING and return the upload_id and device_name.
5. IF an active upload already exists for the same user_id, file_hash, and collection_id, THEN THE Zoo SHALL return HTTP 409 Conflict to prevent duplicate uploads.
6. WHEN a device is deleted via `DELETE /api/devices/me`, THE Zoo SHALL tombstone the device record by setting `archived_at`, cancel all pending uploads from that device, and invalidate the SseToken.

---

### Requirement 13: Upload State Machine

**User Story:** As a user uploading a large file, I want the upload to progress through well-defined states so that failures are detected and recovery is possible.

#### Acceptance Criteria

1. THE Zoo SHALL enforce the following valid state transitions for an upload: PENDING → ENCRYPTING (client-driven), ENCRYPTING → UPLOADING (client-driven), UPLOADING → UPLOADING (heartbeat/progress, client-driven), UPLOADING → S3_COMPLETED (client-driven), S3_COMPLETED → REGISTERING (client-driven), REGISTERING → DONE (server-internal), UPLOADING or ENCRYPTING or any active state → STALLED (Stall_Detector), STALLED → UPLOADING (client resume), STALLED → FAILED (client cancel or GC expiry), UPLOADING → FAILED (S3 error).
2. WHEN a client sends `PATCH /api/uploads/{upload_id}` with a status transition, THE Zoo SHALL validate the transition against the allowed transition table and return HTTP 400 Bad Request if the transition is invalid.
3. WHEN a stalled upload is resumed, THE Zoo SHALL transition it from STALLED directly to UPLOADING (not to ENCRYPTING), because encryption was already completed before the stall.
4. WHEN a client sends `PATCH /api/uploads/{upload_id}` with an updated parts_bitmask, THE Zoo SHALL update the bitmask in the `uploads` table and broadcast an `upload.progress` SSE event to all connected devices of that user.
5. THE Zoo SHALL store the parts_bitmask as a variable-length BYTEA column using a big-endian bit vector encoding where bit N (0-indexed) represents part N: a set bit indicates the part has been uploaded, a clear bit indicates it is pending.
6. WHEN a client transitions an upload to UPLOADING, THE client SHALL send a PATCH heartbeat at least once every 30 seconds, carrying the current parts_bitmask even if no new parts were completed.
7. IF a client sends a state transition that conflicts with the current server state (e.g., transitioning an already-DONE upload), THEN THE Zoo SHALL return HTTP 409 Conflict.

---

### Requirement 14: Multipart Presigning

**User Story:** As a user uploading a large file, I want the server to provide presigned S3 URLs so that encrypted parts are uploaded directly to object storage without passing through the server.

#### Acceptance Criteria

1. WHEN a client calls `POST /api/uploads/{upload_id}/presign` with part_size and an array of part MD5 hashes, THE Zoo SHALL initiate an S3 CreateMultipartUpload, insert one row per part into `upload_parts`, and return the object_key, an array of presigned PUT URLs, the presigned CompleteMultipartUpload URL, and the URL expiry timestamp.
2. WHEN generating presigned URLs, THE Zoo SHALL set the URL expiry to a configurable TTL (default: 24 hours) and store the expiry in `uploads.urls_expire_at`.
3. IF the requested part_count exceeds 10,000 or the part_size is outside the S3-allowed bounds (5 MB minimum, 5 GB maximum per part), THEN THE Zoo SHALL return HTTP 400 Bad Request.
4. WHEN a client calls `POST /api/uploads/{upload_id}/presign-refresh`, THE Zoo SHALL generate new presigned PUT URLs for all parts still marked pending in `upload_parts` and return them with a new expiry timestamp.
5. IF a presigned PUT URL has expired and the client receives an HTTP 403 from S3, THEN THE Zoo_Client SHALL call `presign-refresh` and retry the failed part upload.
6. WHEN all parts have been uploaded to S3, THE Zoo_Client SHALL call the presigned CompleteMultipartUpload URL and then transition the upload to S3_COMPLETED via `PATCH /api/uploads/{upload_id}`.

---

### Requirement 15: Upload Registration and File Record Creation

**User Story:** As a user, I want a successfully uploaded file to be registered in the server's file catalog so that it appears on all my devices after sync.

#### Acceptance Criteria

1. WHEN a client calls `POST /api/uploads/{upload_id}/register` with the encrypted_key, key_decryption_nonce, file_decryption_header, optional thumb_decryption_header, encrypted_metadata, optional encrypted_thumbnail, collection_id, file_size, optional thumbnail_size, and mime_type, THE Zoo SHALL perform a HeadObject call to S3 to verify the object exists and its size matches file_size.
2. IF the S3 HeadObject size does not match the declared file_size, THEN THE Zoo SHALL return HTTP 400 Bad Request and SHALL NOT insert a file record.
3. WHEN the S3 verification succeeds, THE Zoo SHALL insert a row into the `files` table and delete the corresponding rows from `uploads` and `upload_parts`.
4. WHEN the file record is inserted, THE Zoo SHALL broadcast an `upload.done` SSE event containing the upload_id, file_id, and device_name to all connected devices of that user.
5. THE `POST /api/uploads/{upload_id}/register` endpoint SHALL be idempotent: if called again with the same upload_id, THE Zoo SHALL return the same file_id without inserting a duplicate record.
6. WHEN a file is registered, THE Zoo SHALL set `done_at` on the upload record before deleting it, to support debugging.

---

### Requirement 16: Stall Detection

**User Story:** As a user, I want the server to detect when an upload has stalled so that I am notified and can resume or cancel it.

#### Acceptance Criteria

1. THE Stall_Detector SHALL run every 15 seconds and query for uploads with status UPLOADING whose `last_heartbeat_at` is older than the configured stall timeout (default: 90 seconds).
2. WHEN the Stall_Detector identifies a stalled upload, THE Stall_Detector SHALL update the upload status to STALLED, set `stalled_at` to the current timestamp, and broadcast an `upload.stalled` SSE event to all connected devices of that user.
3. WHEN the Stall_Detector identifies a stalled upload and the originating device has a registered push_token, THE Stall_Detector SHALL send a push notification to that device.
4. THE Stall_Detector SHALL use `SELECT ... FOR UPDATE SKIP LOCKED` to prevent multiple Zoo replicas from processing the same stalled upload concurrently.
5. WHERE a device has configured a custom stall timeout (aggressive: 30 s, normal: 90 s, relaxed: 5 min, none), THE Stall_Detector SHALL use that device's configured timeout when evaluating uploads originating from that device.

---

### Requirement 17: Upload Resume Protocol

**User Story:** As a user whose upload was interrupted, I want to resume it from the same device so that I do not have to re-upload already-completed parts.

#### Acceptance Criteria

1. WHEN a client reconnects to the SSE stream after an interruption, THE Zoo SHALL send an `upload.pending` event containing all PENDING and STALLED uploads for that user, including their upload_id, status, parts_bitmask, and part_count.
2. WHEN a client resumes a stalled upload, THE client SHALL send `PATCH /api/uploads/{upload_id}` with `{ status: "resuming" }` to transition the upload from STALLED to UPLOADING.
3. WHEN resuming, THE Zoo_Client SHALL call `GET /api/uploads/{upload_id}` to retrieve the current parts_bitmask, part_count, object_key, and S3 multipart upload ID.
4. WHEN resuming, THE Zoo_Client SHALL call S3 ListParts to retrieve the ground-truth list of uploaded parts and reconcile against the Zoo parts_bitmask using the following rules: if Zoo marks a part uploaded and S3 confirms it with a matching ETag, skip it; if Zoo marks a part uploaded but S3 is missing it or the ETag differs, mark it pending and re-upload; if Zoo marks a part pending and S3 has it, mark it uploaded in Zoo; if Zoo marks a part pending and S3 is missing it, re-upload it.
5. IF S3 returns `NoSuchUpload` during resume (the multipart upload was aborted), THEN THE Zoo_Client SHALL mark the upload as FAILED and start a fresh upload.
6. THE resume protocol SHALL only be possible from the originating device, because the plaintext source file exists only on that device.
7. WHEN the web client stores resume state, THE web client SHALL persist the upload_id in localStorage and, where available, a File System Access API handle to the source file.


---

### Requirement 18: Garbage Collection and Orphan Cleanup

**User Story:** As a system operator, I want orphaned uploads and S3 objects to be cleaned up automatically so that storage costs do not accumulate from abandoned uploads.

#### Acceptance Criteria

1. THE GC_Worker SHALL run every 5 minutes and query for uploads with status NOT IN ('done', 'failed') whose `expires_at` is in the past.
2. WHEN the GC_Worker identifies an expired upload with an S3 multipart upload ID set, THE GC_Worker SHALL call S3 AbortMultipartUpload before marking the upload as FAILED.
3. WHEN the GC_Worker identifies an expired upload with an object_key set but no completed parts in S3, THE GC_Worker SHALL call S3 DeleteObject.
4. WHEN the GC_Worker marks an upload as FAILED, THE GC_Worker SHALL set `error_reason = 'gc_expired'` and broadcast an `upload.failed` SSE event to all connected devices of that user.
5. THE GC_Worker SHALL use `SELECT ... FOR UPDATE SKIP LOCKED` to prevent multiple Zoo replicas from processing the same expired upload concurrently.
6. THE Zoo SHALL enforce the following expiry schedule: PENDING expires 1 hour after creation; ENCRYPTING expires 24 hours after last state transition; UPLOADING expires 24 hours from the current time, reset on each heartbeat; STALLED expires 7 days after `stalled_at`; S3_COMPLETED and REGISTERING expire 1 hour after transition; FAILED records are retained for 24 hours as tombstones.
7. THE Zoo S3 bucket SHALL be configured with a lifecycle rule that aborts incomplete multipart uploads older than 14 days and expires orphaned objects in the `uploads/` prefix after 30 days, as a safety net for GC failures.

---

### Requirement 19: Real-Time SSE Event Stream

**User Story:** As a user with multiple devices, I want real-time upload progress and status events delivered to all my devices so that I always know what is happening across my account.

#### Acceptance Criteria

1. WHEN a client connects to `GET /api/events` with a valid SseToken in the Authorization header, THE Zoo SHALL authenticate the device, subscribe it to the per-user SSE Hub channel, and begin streaming `text/event-stream` responses.
2. WHEN a client connects to the SSE stream, THE Zoo SHALL immediately send an `upload.pending` event containing all current PENDING and STALLED uploads for that user.
3. THE Zoo SHALL send a `heartbeat` event every 15 seconds to all connected clients to keep connections alive through proxies and load balancers.
4. WHEN an upload state changes, THE Zoo SHALL broadcast the appropriate event (`upload.progress`, `upload.completed`, `upload.done`, `upload.stalled`, or `upload.failed`) to all connected devices of the affected user.
5. WHEN a device connects or disconnects from the SSE stream, THE Zoo SHALL broadcast a `device.connected` or `device.disconnected` event to all other connected devices of that user.
6. THE SSE Hub SHALL use a per-user `tokio::broadcast` channel with a buffer capacity of 256 events; if a slow consumer falls behind, THE Zoo SHALL drop events for that consumer rather than blocking the broadcast.
7. WHERE multiple Zoo replicas are deployed, THE Zoo SHALL use PostgreSQL LISTEN/NOTIFY to fan out SSE events across replicas: each replica publishes events via `NOTIFY events, '<json>'` and all replicas listen via `LISTEN events` to forward to their local SSE Hub.
8. IF a client's SseToken is invalid or belongs to an archived device, THEN THE Zoo SHALL return HTTP 401 Unauthorized and close the connection.

---

### Requirement 20: File Download

**User Story:** As a user, I want to download and decrypt my files so that I can view them in full resolution on any of my devices.

#### Acceptance Criteria

1. WHEN a client calls `GET /api/files/{file_id}/download`, THE Zoo SHALL verify that the file exists in the `files` table and that `file.user_id` matches the authenticated user's user_id.
2. IF the file record has `archived_at` set, THEN THE Zoo SHALL return HTTP 403 Forbidden.
3. IF the file does not belong to the authenticated user and no valid share record exists, THEN THE Zoo SHALL return HTTP 403 Forbidden.
4. WHEN download access is verified in Redirect mode (default), THE Zoo SHALL generate a presigned S3 GET URL with a 7-day TTL and return HTTP 302 with the URL in the Location header.
5. WHEN download access is verified in Proxy mode, THE Zoo SHALL stream the encrypted object bytes from S3 through itself to the client with `Content-Type: application/octet-stream`.
6. WHEN a client receives encrypted file bytes, THE Crypto_Module SHALL decrypt them using XChaCha20-Poly1305 with the FileKey and the stored file_decryption_header.
7. THE download mode (Redirect or Proxy) SHALL be configurable per Zoo deployment without requiring code changes.

---

### Requirement 21: Client-Side Text Search

**User Story:** As a user, I want to search my photos by title, description, date, and location so that I can find specific photos quickly.

#### Acceptance Criteria

1. THE Local_DB SHALL support full-text search against the `title` and `description` columns of the `files` table using SQLite FTS5 virtual tables, so that search queries use indexed lookups rather than full table scans.
2. WHEN a user submits a text search query, THE Local_DB SHALL execute an FTS5 query against the `title` and `description` columns, filtered to non-archived files, ordered by `taken_at` descending, with a limit of 50 results.
3. WHEN a user submits a date range search, THE Local_DB SHALL execute a query filtering `taken_at BETWEEN start_timestamp AND end_timestamp` on non-archived files, ordered by `taken_at` descending.
4. WHEN a user submits a geographic bounding-box search, THE Local_DB SHALL execute a query filtering `latitude BETWEEN lat_min AND lat_max AND longitude BETWEEN lon_min AND lon_max` on non-archived files.
5. THE Zoo SHALL NOT expose any search endpoints; all search operations SHALL execute entirely against the Local_DB on the client device.
6. WHEN the Local_DB FTS5 index is out of sync with the `files` table (e.g., after a bulk sync), THE Local_DB SHALL rebuild the FTS5 index before returning search results.

---

### Requirement 22: Tauri Desktop Platform Support

**User Story:** As a desktop user on macOS, Windows, or Linux, I want a native application so that I get full OS integration including keychain access, file system access, and background sync.

#### Acceptance Criteria

1. THE Tauri desktop application SHALL compile and run on macOS (arm64 and x86_64), Windows (x86_64), and Linux (x86_64) from a single Rust codebase.
2. WHEN the Tauri application starts, THE client-lib crate SHALL initialize the Local_DB, load the sync cursors, and begin an incremental sync in the background.
3. THE Tauri application SHALL expose all business logic through Tauri IPC commands defined in `client-lib::commands`, and the frontend SHALL invoke these commands via `invoke()` without containing any business logic.
4. THE Tauri application SHALL use `tauri_specta` to generate fully typed TypeScript bindings from Rust command definitions, so that TypeScript types are always consistent with Rust types.
5. WHEN the Tauri application encrypts or decrypts data, THE Crypto_Module SHALL execute as native Rust code within the Tauri process, not via WASM.
6. THE Tauri application SHALL store the Local_DB encryption key in the platform credential store: macOS Keychain on macOS, Windows DPAPI on Windows, and the Linux secret-service on Linux.
7. THE Tauri application SHALL hold the MasterKey in secret-protected memory and SHALL NOT write it to disk, swap, or any persistent store.

---

### Requirement 23: Web Platform Support

**User Story:** As a web user, I want to access my encrypted photos in a browser so that I can view and manage my library without installing a desktop application.

#### Acceptance Criteria

1. THE web application SHALL compile the `crypto` crate to WebAssembly via `wasm-pack` and expose encryption and decryption operations through the `zoo-wasm` crate's `#[wasm_bindgen]` interface.
2. WHEN the web application encrypts or decrypts data, THE Crypto_Module SHALL execute within the browser's WASM runtime, not on the server.
3. THE web application SHALL use the `zoo-wasm` crate to drive the upload state machine, including multipart presigning, S3 PUT, and registration.
4. WHEN the web application uploads a file, THE web client SHALL persist the upload_id in localStorage so that the upload can be resumed after a page reload.
5. THE web application SHALL connect to the Zoo SSE stream using the device's SseToken and display real-time upload progress.
6. THE Zoo server SHALL generate an OpenAPI specification from handler types using `utoipa`, and the TypeScript API client in `packages/api-client` SHALL be auto-generated from that specification, so that TypeScript types are always consistent with server types.
7. THE web application and Tauri desktop application SHALL share the same frontend component library from `packages/ui`.

---

### Requirement 24: Zero-Knowledge Server Guarantee

**User Story:** As a privacy-conscious user, I want a guarantee that the server never has access to my plaintext content or encryption keys so that a server breach does not expose my photos.

#### Acceptance Criteria

1. THE Zoo SHALL never receive, store, or process any plaintext file data, plaintext metadata, plaintext thumbnail pixels, KEK, MasterKey, CollectionKey, or FileKey.
2. THE Zoo SHALL store only ciphertext blobs in S3, encrypted keys and nonces in the `files` table, and `bcrypt(VerifyKeyHash)` in the sessions/users table.
3. THE Zoo crate SHALL NOT list `crypto`, `image`, `metadata`, or `thumbnail` as Cargo dependencies, enforced at compile time by the workspace dependency graph.
4. THE `zoo-wasm` crate SHALL NOT list `crypto`, `image`, `metadata`, or `thumbnail` as Cargo dependencies, enforced at compile time.
5. WHEN the Zoo server is breached, the attacker SHALL obtain only ciphertext, bcrypt hashes of VerifyKeyHashes, and encrypted keys — none of which yield plaintext without reversing Argon2id with the original password.
6. THE content_hash (SHA-256 of plaintext) stored in the `files` table is a known metadata leak: the server can determine if two users have uploaded identical files. THE Zoo SHALL document this limitation and SHALL NOT use content_hash for any purpose other than client-side deduplication detection.


---

### Requirement 25: Crate Architecture and Dependency Isolation

**User Story:** As a developer, I want the crate boundaries to be enforced at compile time so that server code can never accidentally link crypto primitives and client code can never accidentally link server frameworks.

#### Acceptance Criteria

1. THE workspace SHALL be organized as a virtual manifest with all crates under `crates/`, and the dependency graph SHALL be a directed acyclic graph with no cycles.
2. THE `types` crate SHALL depend only on `serde` and `serde_json` and SHALL contain no I/O, no HTTP, no database, and no crypto logic.
3. THE `crypto` crate SHALL depend only on `types` and cryptographic primitive crates (`aead`, `xchacha20poly1305`, `xsalsa20poly1305`, `x25519-dalek`, `argon2`, `blake2b_simd`, `rand_core`) and SHALL be `#![no_std]` compatible.
4. THE `zoo` server crate SHALL NOT list `crypto`, `image`, `metadata`, `thumbnail`, `sync`, `local-db`, or `client-lib` as dependencies in its `Cargo.toml`.
5. THE `client-lib` crate SHALL NOT list `axum`, `sqlx` (postgres), `aws-sdk-s3`, or any server-side framework as dependencies in its `Cargo.toml`.
6. THE `zoo-client` crate SHALL NOT list `crypto`, `image`, `metadata`, or `thumbnail` as dependencies, and SHALL operate on encrypted bytes only without knowledge of the encryption scheme.
7. WHEN any crate violates the import rules defined in STRUCTURE.md, THE Cargo build SHALL fail with a dependency resolution error, preventing the violation from being compiled.

---

### Requirement 26: EXIF Metadata Extraction

**User Story:** As a user, I want my photos' location, date, and camera information to be extracted automatically so that I can search and organize them without manual tagging.

#### Acceptance Criteria

1. WHEN a file is prepared for upload, THE `image` crate SHALL extract EXIF metadata including GPS coordinates (latitude, longitude), capture datetime (taken_at), device make, device model, and orientation tag.
2. WHEN EXIF orientation data is present, THE `image` crate SHALL apply orientation correction to the decoded image before thumbnail generation or encoding.
3. WHEN EXIF GPS coordinates are present, THE `metadata` crate SHALL include latitude and longitude in the FileMetadata struct that is encrypted with the FileKey.
4. IF a file has no EXIF data or the EXIF data is malformed, THEN THE `image` crate SHALL return a partial result with available fields populated and missing fields set to null, without returning an error.
5. THE extracted EXIF metadata SHALL be encrypted as part of the FileMetadata struct using XChaCha20-Poly1305 with the FileKey before transmission to the server.

---

### Requirement 27: Upload Cancellation

**User Story:** As a user, I want to cancel an in-progress or stalled upload so that I can free up resources and remove it from my upload queue.

#### Acceptance Criteria

1. WHEN a client calls `DELETE /api/uploads/{upload_id}`, THE Zoo SHALL verify that the upload belongs to the authenticated user and return HTTP 404 if not found.
2. WHEN an upload is cancelled and an S3 multipart upload ID is set, THE Zoo SHALL call S3 AbortMultipartUpload before marking the upload as FAILED.
3. WHEN an upload is cancelled and an object_key is set but no parts have been completed in S3, THE Zoo SHALL call S3 DeleteObject.
4. WHEN an upload is cancelled, THE Zoo SHALL set the upload status to FAILED and broadcast an `upload.failed` SSE event with reason "cancelled" to all connected devices of that user.
5. WHEN a client cancels an upload, THE Zoo_Client SHALL remove the upload_id from localStorage (web) or the local state store (desktop).

---

### Requirement 28: Upload Queue Visibility

**User Story:** As a user, I want to see all pending and stalled uploads across all my devices so that I have full visibility into what is being uploaded.

#### Acceptance Criteria

1. WHEN a client calls `GET /api/uploads?status=active`, THE Zoo SHALL return all uploads for the authenticated user with status in (PENDING, ENCRYPTING, UPLOADING, S3_COMPLETED, REGISTERING).
2. WHEN a client calls `GET /api/uploads?status=stalled`, THE Zoo SHALL return all uploads for the authenticated user with status STALLED.
3. WHEN a client calls `GET /api/uploads?status=all`, THE Zoo SHALL return all uploads for the authenticated user regardless of status.
4. WHEN a client calls `GET /api/uploads/{upload_id}`, THE Zoo SHALL return the full UploadState including the parts list, parts_bitmask, part_count, object_key, and S3 multipart upload ID.
5. THE upload list response SHALL include the device_name for each upload so that the user can identify which device originated each upload.

