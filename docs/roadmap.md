# Implementation Roadmap

## Phase 1: Upload Pipeline
1. **Server: Presigned URL endpoint** - Generate S3 presigned URLs for uploads
2. **Server: File registration endpoint** - Register file metadata after upload completes
3. **Frontend: Thumbnail generation** - Canvas-based resize, encrypt, upload alongside file
4. **Frontend: Upload manager** - File picker, queue, progress tracking, concurrent uploads
5. **Frontend: Chunked encryption** - Encrypt file in chunks using WASM, upload parts

## Phase 2: Gallery Display
6. **Server: File list endpoint** - Return encrypted file list with pagination
7. **Frontend: Masonry grid** - Responsive masonry layout with lazy-loaded thumbnails
8. **Frontend: Photo viewer** - Click to view, progressive loading (thumb → original)

## Phase 3: Collections & Sync
9. **Server: Collections CRUD** - Create, list, add/remove files
10. **Frontend: Collections UI** - Album creation, file assignment
11. **Server: Delta sync** - Timestamp-based diff endpoint
12. **Frontend: Sync engine** - Local IndexedDB cache, background sync

## Current State
- ✅ Auth flow (zero-knowledge, 2-step login)
- ✅ WASM crypto module (Argon2, X25519, XSalsa20, BLAKE2b, bcrypt)
- ✅ Session validation + route gating
- ❌ Upload pipeline
- ❌ Thumbnail generation
- ❌ Gallery display
- ❌ Collections
- ❌ Sync
