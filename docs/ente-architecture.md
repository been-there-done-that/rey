# Ente Architecture Deep Dive

## 1. UPLOAD FLOW

### Pipeline Overview

```
User selects file(s)
       |
       v
[UploadManager]
  - Prepares upload queue
  - Spawns maxConcurrentUploads = 4 workers
  - Clusters live photos (image + video pairing)
       |
       v
[UploadService.upload()]
  ├── Step 1: Read asset details (file type, size)
  │     - detectFileTypeInfoFromChunk() - reads first few KB
  │
  ├── Step 2: Extract metadata (EXIF, video duration, location)
  │     - extractImageOrVideoMetadata()
  │     - Try EXIF, fallback to JSON metadata (Google Takeout)
  │
  ├── Step 3: Compute hash for deduplication
  │     - computeHash() - Chunked BLAKE2b hash via crypto worker
  │
  ├── Step 4: Deduplication check
  │     - areFilesSame() - Compares: fileName + fileType + metadataHash
  │     - If match in same collection -> "alreadyUploaded"
  │     - If match in different collection -> "addedSymlink"
  │
  ├── Step 5: Generate thumbnail (CLIENT-SIDE)
  │     - readAsset() -> augmentWithThumbnail()
  │     - Images: Canvas-based resize to max 720px
  │     - Videos: FFmpeg WASM worker, fallback to canvas
  │     - HEIC: Pre-convert to JPEG before canvas
  │     - Adaptive JPEG compression (quality 0.7 → 0.5)
  │       until size < 100 KB
  │
  ├── Step 6: Encrypt file + thumbnail + metadata
  │     - encryptFile()
  │     - Uses crypto_secretstream (XChaCha20-Poly1305)
  │     - 4 MB chunk size (streamEncryptionChunkSize)
  │     - Generates fileKey, encrypts with collectionKey
  │
  ├── Step 7: Upload to S3
  │     - uploadToBucket()
  │     - Small files: Single PUT to presigned URL
  │     - Large files (>5 chunks = 20 MB): Multipart upload
  │       * Each part = 5 chunks = 20 MB
  │       * Fetch presigned URLs from server
  │       * Upload parts in parallel
  │       * Complete multipart with XML manifest
  │     - Optional: Route via Cloudflare worker proxy
  │
  └── Step 8: Create remote file record
        - postEnteFile() → POST /files
        - Sends: objectKey, decryptionHeader, encryptedKey,
          keyDecryptionNonce, metadata, pubMagicMetadata
```

### Key Files

| Component | Path | Lines |
|-----------|------|-------|
| Upload Manager | `web/apps/photos/src/services/upload-manager.ts` | 253-788 |
| Upload Service | `web/packages/gallery/services/upload/upload-service.ts` | 131-1800+ |
| Thumbnail generation | `web/packages/gallery/services/upload/thumbnail.ts` | 1-239 |
| Encryption (Rust) | `rust/core/src/crypto/impl_pure/stream.rs` | 1-1452 |
| Server file handler | `server/pkg/api/file.go` | 38-470 |
| Server file controller | `server/pkg/controller/file.go` | 131-200+ |

### Chunking / Multipart Upload
- **Chunk size**: 4 MB (`streamEncryptionChunkSize`)
- **Multipart threshold**: >5 chunks (20 MB)
- **Part size**: 5 chunks per part = 20 MB
- **Max parts**: 10,000 (S3 limit)
- **Server enforces**: min 5 MB, max 5 GB per part

### Client-Side Encryption
- **Algorithm**: XChaCha20-Poly1305 via `crypto_secretstream`
- **Key hierarchy**: `masterKey → collectionKey → fileKey → file data`
- **Chunked streaming**: 4 MB plaintext chunks, each encrypted with TAG_MESSAGE, last with TAG_FINAL
- **Rust implementation**: `rust/core/src/crypto/impl_pure/stream.rs`
- **Web worker**: Crypto operations run in Web Workers via Comlink

### Upload Progress Tracking
- Per-file progress: 0-100%
- Overall progress: weighted average of all files
- Upload phases: "preparing" → "readingMetadata" → "uploading" → "done"
- Memory pressure monitoring: logs when JS heap > 70% of limit

### Deduplication
- **Hash-based**: BLAKE2b chunked hash of file contents
- **Comparison**: `fileName + fileType + metadataHash`
- **Symlink approach**: If file exists in another collection, add a symlink instead of re-uploading
- **Server-side**: Also detects duplicate object keys

---

## 2. THUMBNAIL GENERATION

### Where: CLIENT-SIDE ONLY

Ente generates thumbnails entirely on the client. The server stores them but never creates them.

### Thumbnail Specifications
- **Max dimension**: 720px
- **Max size**: 100 KB
- **Format**: JPEG (universal compatibility)
- **Image generation**: HTML Canvas with `drawImage()` + `toBlob("image/jpeg", quality)`
- **Video generation**: FFmpeg WASM worker, fallback to canvas `<video>` element
- **HEIC handling**: Pre-convert to JPEG before canvas rendering

### Thumbnail Generation Algorithm
```
1. Load image/video into browser
2. Calculate scaled dimensions (max 720px)
3. Draw to canvas
4. Compress as JPEG starting at quality 0.7
5. Iteratively reduce quality (0.6, 0.5) if size > 100 KB
6. Stop when size < 100 KB or quality reaches 0.5
```

### Storage and Serving
- **Storage**: Uploaded to S3 as separate object with its own `objectKey`
- **Serving**:
  - Ente.com: Dedicated thumbnail CDN (`https://thumbnails.ente.com/?fileID=${fileID}`)
  - Self-hosted: Redirect to presigned S3 URL
- **Caching**: IndexedDB blob cache (`blobCache("thumbs")`)
- **Download**: Decrypted client-side using `decryptBlobBytes()` with file's key

---

## 3. PHOTO VIEWING / DISPLAY

### Click-to-View Flow

```
User clicks thumbnail
       |
       v
[FileViewer] opens
  - Uses PhotoSwipe library for image/video viewer
  - Receives list of files + initial index
       |
       v
[FileViewerDataSource]
  - itemDataForFile() called for each slide
  - Progressive loading:
    1. Return empty placeholder (loading state)
    2. Fetch thumbnail → update slide
    3. Fetch original → update slide
       |
       v
[DownloadManager]
  - renderableThumbnailURL() → cached or download
  - renderableSourceURLs() → full resolution
  - Streaming decryption for videos
       |
       v
[Display in PhotoSwipe]
  - Images: <img src={imageURL}>
  - Videos: HLS streaming or direct <video>
  - Live Photos: image + video components
```

### Grid Layout
- **Layout calculation**: `web/packages/new/photos/components/utils/thumbnail-grid-layout.ts`
- **Algorithm**: Responsive column-based grid
  - `thumbnailMaxHeight = 180px`, `thumbnailMaxWidth = 180px`
  - `thumbnailLayoutMinColumns = 4`
  - Columns calculated from container width
  - Shrink ratio applied to fit columns
  - Gap: 4px between items, 24px from screen edge (4px on small screens)
- **NOT true masonry**: It is a uniform grid with calculated item dimensions

### Lazy Loading
- **Thumbnails**: Loaded on-demand via `renderableThumbnailURL(file, cachedOnly)`
  - During scroll: `cachedOnly=true` to avoid request storms
  - After scroll quiesces: full download
- **Full resolution**: Only fetched when user opens file viewer
- **In-memory cache**: `thumbnailURLPromises` and `fileURLPromises` maps

### Virtualization
- **PhotoSwipe**: Handles virtualization of slides (preloads adjacent slides)
- **No custom virtualization**: The gallery itself does not appear to use windowing/virtualization for the thumbnail grid - all thumbnails are rendered in the DOM
- **Preload range**: PhotoSwipe preloads slides at index `i-1` and `i+1`

---

## 4. METADATA HANDLING

### Metadata Layers

Ente uses a **three-tier metadata system**:

```
1. File Metadata (immutable, encrypted with fileKey)
   - fileType, title, hash, creationTime, modificationTime
   - latitude, longitude, duration

2. Public Magic Metadata (mutable, encrypted with collectionKey)
   - Visible to anyone with collection access
   - w, h (dimensions), caption, dateTime, offsetTime
   - cameraMake, cameraModel
   - Collection: asc (sort order), coverID, layout

3. Private Magic Metadata (mutable, encrypted with collectionKey)
   - Only visible to owner
   - visibility (hidden/archived/visible)
   - order (pinned)
   - subType (quicklink, defaultHidden)

4. Sharee Magic Metadata (per-sharee, encrypted with collectionKey)
   - Per-user visibility and order preferences
```

### EXIF Handling
- **Extraction**: `ExifReader` library for images, FFmpeg for videos
- **Storage**: EXIF extracted client-side, stored in `pubMagicMetadata`
- **Viewing**: Extracted again from original blob when viewing
- **Caching**: EXIF cached per file ID in `FileViewerDataSourceState`

### External Metadata (Google Takeout, etc.)
- JSON metadata files parsed separately (`metadata-json.ts`)
- Matched to files by filename + path prefix
- Provides: creationTime, modificationTime, location, description

### Albums/Collections
- **Collection model**: Each collection has a `collectionKey` encrypted with `masterKey`
- **File-to-collection**: Files can exist in multiple collections (symlinks)
- **Sync**: Per-collection `sinceTime` tracking for efficient diff sync

---

## 5. SYNC ARCHITECTURE

### Sync Flow

```
Client starts sync
       |
       v
[prePullFiles]
  - Pull settings, ML status
       |
       v
[pullFiles]
  ├── pullCollections()
  │     - GET /collections/v2?sinceTime=X
  │     - Delta sync: only changes since last sync
  │     - Decrypt each collection with collectionKey
  │     - Save to IndexedDB (photos-fdb)
  │
  ├── pullCollectionFiles()
  │     - For each collection:
  │       * GET /collections/v2/diff?collectionID=X&sinceTime=Y
  │       * Paginated with hasMore flag
  │       * Decrypt each file with collection's key
  │       * Clear cached thumbnail if content changed
  │       * Save to IndexedDB
  │
  └── pullTrash()
        - Sync trash items
       |
       v
[postPullFiles]
  - Search data sync
  - Video processing sync
  - ML sync (async, non-blocking)
```

### Key Design Patterns

**1. Timestamp-based delta sync**
- Each entity has `updationTime` (epoch microseconds)
- Client stores `sinceTime` of last successful sync
- Server returns only entities with `updationTime > sinceTime`
- Collection files sync per-collection, not global

**2. Version-based consistency**
- Server ensures no partial results for a version
- If limit cuts mid-version, returns all items of that version
- Prevents client from seeing inconsistent state

**3. Local-first with remote reconciliation**
- IndexedDB (photos-fdb) is the source of truth for UI
- Remote pull updates local DB
- Local changes pushed to remote, then pulled back

**4. Multi-layer encryption**
- Server never sees plaintext data
- Collection keys encrypted with masterKey (owned) or publicKey (shared)
- File keys encrypted with collectionKey
- All metadata encrypted at appropriate layer

---

## WHAT ENTE DOES RIGHT

1. **End-to-end encryption done properly**: Clear key hierarchy, audited cryptography, libsodium-compatible wire format
2. **Streaming encryption**: 4 MB chunks with XChaCha20-Poly1305, memory-bounded (2x chunk size)
3. **Client-side thumbnail generation**: Universal JPEG format, adaptive compression, works across all platforms
4. **Progressive loading**: Thumbnail first, then original - excellent UX
5. **Delta sync with version consistency**: No partial results, per-collection tracking
6. **Deduplication via symlinks**: Same file in multiple collections without re-uploading
7. **Rust crypto core**: Pure Rust implementation with zeroize-on-drop, shared across web/mobile/desktop
8. **Multipart upload resilience**: Independent part retries, CF worker proxy option
9. **Live photo support**: Intelligent pairing by filename + timestamp proximity
10. **Three-tier metadata system**: Immutable file metadata + mutable public/private/sharee metadata

---

## WHAT WE MIGHT BE MISSING

1. **Gallery virtualization**: Ente renders all thumbnails in DOM. For large libraries, consider windowing/virtualization (react-window, tanstack-virtual)
2. **True masonry layout**: Ente uses uniform grid. Pinterest-style masonry requires column-based layout with varying heights
3. **Thumbnail variants**: Only one thumbnail size (720px). Consider multiple sizes for different viewport densities
4. **Background sync**: Ente syncs on app focus/5-min intervals. Consider Service Worker-based background sync
5. **Offline-first architecture**: Ente caches thumbnails but not originals. Consider full offline mode with IndexedDB for originals
6. **WebP/AVIF thumbnails**: JPEG is universal but WebP/AVIF offer 30-50% size reduction
7. **Thumbnail CDN**: Ente uses dedicated thumbnail subdomain. Consider imgproxy or Cloudflare Image Resizing
8. **Upload resumption**: Ente retries failed uploads but does not resume interrupted multipart uploads
9. **Perceptual hash deduplication**: Ente uses exact hash. Consider pHash for near-duplicate detection
10. **Server-side ML**: Ente does face detection/CLIP embedding client-side. Consider server-side for desktop/mobile consistency

---

## PATTERNS WORTH ADOPTING

1. **Key hierarchy pattern**: `masterKey → collectionKey → fileKey` provides clean sharing semantics
2. **Magic metadata pattern**: Separate encrypted metadata layers (public/private/sharee) for different visibility scopes
3. **Timestamp-based delta sync**: Simple, efficient, works well with SQL databases
4. **Version-based diff consistency**: Never return partial results for a version
5. **Streaming encryption with fixed chunks**: Bounded memory, resumable, parallelizable
6. **Symlink deduplication**: Same file in multiple collections without storage duplication
7. **Progressive file loading**: Thumbnail → Original → EXIF extraction pipeline
8. **Comlink-based crypto workers**: Clean Web Worker communication pattern
9. **Blob cache with IndexedDB**: Persistent thumbnail cache across sessions
10. **Rust crypto core with WASM/FRB bindings**: Single crypto implementation across all platforms

---

## OUR CURRENT STATE vs ENTE

| Feature | Ente | Us (rey) | Gap |
|---------|------|----------|-----|
| Client-side encryption | XChaCha20-Poly1305 streaming | XSalsa20 via WASM | ✅ Similar approach |
| Thumbnail generation | Canvas + FFmpeg WASM | None yet | ❌ Missing |
| Upload pipeline | Chunked, multipart, progress | None yet | ❌ Missing |
| Deduplication | BLAKE2b hash + symlink | None yet | ❌ Missing |
| Gallery layout | Responsive grid | Empty placeholder | ❌ Missing |
| Photo viewer | PhotoSwipe, progressive load | None yet | ❌ Missing |
| Delta sync | Timestamp-based, per-collection | None yet | ❌ Missing |
| Metadata layers | 3-tier encrypted | None yet | ❌ Missing |
| Collections/Albums | Yes, with sharing | None yet | ❌ Missing |
| IndexedDB cache | Thumbnails + files | None yet | ❌ Missing |
| Auth flow | Zero-knowledge, 2-step login | ✅ Implemented | ✅ Done |
| WASM crypto | Rust core, shared | ✅ Rust core | ✅ Done |
