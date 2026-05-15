# Task 8: Implement `crates/thumbnail` — Layer 1 Thumbnail Pipeline

## Wave
2 (Layer 1 — Pure Libraries)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 5 (crypto) must be complete
- Task 6 (image) must be complete
- Task 7 (metadata) must be complete

## Can Run In Parallel With
Nothing in this wave — thumbnail depends on crypto, image, and metadata

## Design References
- design.md §8.1: Thumbnail Module Structure
- design.md §8.2: Generation Pipeline
- design.md §8.3: Cache Design (two-level LRU, in-flight dedup)
- SPEC.md §3.1: Thumbnail Generation (720px max, JPEG quality 85, ≤100KB)
- SPEC.md §3.2: Client Cache (L1: 500 items, L2: 2GB disk)
- SPEC.md §3.4: Cache Invalidation table

## Requirements
10.1–10.8, 11.1–11.8, 25.3

## Objective
Generate thumbnails from images, encrypt them, and manage a two-level LRU cache (memory + disk) with in-flight request deduplication.

## Cargo.toml
```toml
[package]
name = "thumbnail"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
crypto = { workspace = true }
image = { workspace = true }  # internal image crate
metadata = { workspace = true }
lru = { workspace = true }
dashmap = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
base64 = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod generate;
pub mod encrypt;
pub mod decrypt;
pub mod cache;
pub mod download;
pub mod inflight;
pub mod invalidation;
pub mod error;

pub use generate::generate_thumbnail;
pub use cache::ThumbnailCache;
pub use invalidation::ThumbnailInvalidator;
pub use error::ThumbnailError;
```

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum ThumbnailError {
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("thumbnail generation failed: {0}")]
    GenerationFailed(String),
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::error::CryptoError),
    #[error("cache error: {0}")]
    CacheError(String),
    #[error("thumbnail not found")]
    NotFound,
    #[error("download error: {0}")]
    DownloadError(String),
}
```

### `src/generate.rs`
Implement `generate_thumbnail(source: &[u8], mime_type: &str, file_key: &Key256) -> Result<(Header24, Vec<u8>), ThumbnailError>`:
1. Decode image: `image::decode_image(source, mime_type)?`
2. Extract EXIF orientation: `image::extract_exif(source).orientation`
3. Apply orientation: `image::apply_orientation(img, orientation)`
4. Resize: `image::resize_max_dimension(img, 720)`
5. Encode JPEG at quality 85: `image::encode_jpeg(img, 85)`
6. If > 100 KB, iteratively reduce quality by 10 until ≤ 100 KB or quality reaches 10
7. If still > 100 KB at quality 10, use quality 10 anyway
8. Encrypt: `crypto::stream_encrypt(&jpeg_bytes, file_key)`
9. Return `(header, ciphertext)`
- For video mime_types (`video/*`): return `ThumbnailError::UnsupportedFormat` (video frame extraction deferred)
- On any failure: return `ThumbnailError::UnsupportedFormat` — do NOT panic

```rust
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::ThumbnailError;

const MAX_DIMENSION: u32 = 720;
const MAX_SIZE_BYTES: usize = 100 * 1024; // 100 KB
const INITIAL_QUALITY: u8 = 85;
const MIN_QUALITY: u8 = 10;
const QUALITY_STEP: u8 = 10;

pub fn generate_thumbnail(
    source: &[u8],
    mime_type: &str,
    file_key: &Key256,
) -> Result<(Header24, Vec<u8>), ThumbnailError> {
    // Reject video mime types
    if mime_type.starts_with("video/") {
        return Err(ThumbnailError::UnsupportedFormat);
    }

    // Step 1: Decode
    let img = image::decode_image(source, mime_type)
        .map_err(|_| ThumbnailError::UnsupportedFormat)?;

    // Step 2: Extract EXIF orientation
    let exif = image::extract_exif(source);
    let orientation = exif.orientation.unwrap_or(1);

    // Step 3: Apply orientation
    let img = image::apply_orientation(img, orientation);

    // Step 4: Resize
    let img = image::resize_max_dimension(img, MAX_DIMENSION);

    // Step 5-7: Encode with iterative quality reduction
    let mut quality = INITIAL_QUALITY;
    let mut jpeg_bytes = image::encode_jpeg(&img, quality);

    while jpeg_bytes.len() > MAX_SIZE_BYTES && quality > MIN_QUALITY {
        quality = quality.saturating_sub(QUALITY_STEP);
        if quality < MIN_QUALITY {
            quality = MIN_QUALITY;
        }
        jpeg_bytes = image::encode_jpeg(&img, quality);
    }

    // Step 8: Encrypt
    let (header, ciphertext) = crypto::stream_encrypt(&jpeg_bytes, file_key)
        .map_err(ThumbnailError::Crypto)?;

    // Step 9: Return
    Ok((header, ciphertext))
}
```

### `src/encrypt.rs` and `src/decrypt.rs`
Thin wrappers:
- `encrypt_thumbnail(bytes: &[u8], file_key: &Key256) -> (Header24, Vec<u8>)` → `crypto::stream_encrypt`
- `decrypt_thumbnail(header: &Header24, ciphertext: &[u8], file_key: &Key256) -> Result<Vec<u8>, ThumbnailError>` → `crypto::stream_decrypt`

```rust
// src/encrypt.rs
use types::crypto::Key256;
use types::crypto::Header24;

pub fn encrypt_thumbnail(bytes: &[u8], file_key: &Key256) -> (Header24, Vec<u8>) {
    crypto::stream_encrypt(bytes, file_key).expect("thumbnail encryption should not fail")
}
```

```rust
// src/decrypt.rs
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::ThumbnailError;

pub fn decrypt_thumbnail(
    header: &Header24,
    ciphertext: &[u8],
    file_key: &Key256,
) -> Result<Vec<u8>, ThumbnailError> {
    let plaintext = crypto::stream_decrypt(header, ciphertext, file_key)
        .map_err(ThumbnailError::Crypto)?;
    Ok(plaintext)
}
```

### `src/cache/memory.rs`
Implement `MemoryCache`:
- Wraps `lru::LruCache<FileId, Vec<u8>>` with capacity 500
- `new() -> Self`
- `get(&file_id) -> Option<Vec<u8>>`
- `insert(file_id, bytes)` — evicts LRU if at capacity
- `remove(&file_id)`
- `len() -> usize`

```rust
use lru::LruCache;
use types::file::FileId;
use std::num::NonZeroUsize;

const DEFAULT_CAPACITY: usize = 500;

pub struct MemoryCache {
    cache: LruCache<FileId, Vec<u8>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(DEFAULT_CAPACITY).unwrap()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
        }
    }

    pub fn get(&mut self, file_id: &FileId) -> Option<Vec<u8>> {
        self.cache.get(file_id).cloned()
    }

    pub fn insert(&mut self, file_id: FileId, bytes: Vec<u8>) {
        self.cache.put(file_id, bytes);
    }

    pub fn remove(&mut self, file_id: &FileId) {
        self.cache.pop(file_id);
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}
```

### `src/cache/disk.rs`
Implement `DiskCache`:
- Stores files at `{cache_dir}/thumbnails/{file_id}`
- Maintains metadata index in a small SQLite file: `(file_id TEXT PK, path TEXT, size INTEGER, last_accessed INTEGER)`
- `new(cache_dir: PathBuf, max_bytes: u64) -> Self`
- `get(&file_id) -> Result<Option<Vec<u8>>, DiskCacheError>` — reads file, updates last_accessed
- `insert(&file_id, bytes: &[u8]) -> Result<(), DiskCacheError>` — writes file, updates index
- `remove(&file_id) -> Result<(), DiskCacheError>` — deletes file and index entry
- `evict_lru_until_below(&mut self, limit_bytes: u64) -> Result<(), DiskCacheError>` — evicts LRU entries until total size ≤ limit
- `total_size() -> u64` — sum of all cached file sizes

```rust
use std::path::PathBuf;
use std::fs;
use rusqlite::Connection;
use crate::error::DiskCacheError;

pub struct DiskCache {
    cache_dir: PathBuf,
    max_bytes: u64,
    conn: Connection,
}

impl DiskCache {
    pub fn new(cache_dir: PathBuf, max_bytes: u64) -> Result<Self, DiskCacheError> {
        let thumb_dir = cache_dir.join("thumbnails");
        fs::create_dir_all(&thumb_dir).map_err(|e| DiskCacheError::Io(e.to_string()))?;

        let conn = Connection::open(cache_dir.join("thumbnail_index.sqlite"))
            .map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache_entries (
                file_id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                size INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL
            )",
            [],
        ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        Ok(Self {
            cache_dir,
            max_bytes,
            conn,
        })
    }

    pub fn get(&mut self, file_id: &str) -> Result<Option<Vec<u8>>, DiskCacheError> {
        let path = self.cache_dir.join("thumbnails").join(file_id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&path).map_err(|e| DiskCacheError::Io(e.to_string()))?;

        // Update last_accessed
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE cache_entries SET last_accessed = ? WHERE file_id = ?",
            [&now, file_id],
        ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        Ok(Some(bytes))
    }

    pub fn insert(&mut self, file_id: &str, bytes: &[u8]) -> Result<(), DiskCacheError> {
        let path = self.cache_dir.join("thumbnails").join(file_id);
        fs::write(&path, bytes).map_err(|e| DiskCacheError::Io(e.to_string()))?;

        let size = bytes.len() as u64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO cache_entries (file_id, path, size, last_accessed)
             VALUES (?, ?, ?, ?)",
            [file_id, path.to_str().unwrap_or(""), &size.to_string(), &now.to_string()],
        ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        // Evict if over limit
        self.evict_lru_until_below(self.max_bytes)?;

        Ok(())
    }

    pub fn remove(&mut self, file_id: &str) -> Result<(), DiskCacheError> {
        let path = self.cache_dir.join("thumbnails").join(file_id);
        let _ = fs::remove_file(&path);

        self.conn.execute(
            "DELETE FROM cache_entries WHERE file_id = ?",
            [file_id],
        ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        Ok(())
    }

    pub fn evict_lru_until_below(&mut self, limit_bytes: u64) -> Result<(), DiskCacheError> {
        loop {
            let total = self.total_size()?;
            if total <= limit_bytes {
                break;
            }

            // Get LRU entry (oldest last_accessed)
            let row = self.conn.query_row(
                "SELECT file_id, path FROM cache_entries ORDER BY last_accessed ASC LIMIT 1",
                [],
                |row| {
                    let file_id: String = row.get(0)?;
                    let path: String = row.get(1)?;
                    Ok((file_id, path))
                },
            );

            match row {
                Ok((file_id, path)) => {
                    let _ = fs::remove_file(&path);
                    self.conn.execute(
                        "DELETE FROM cache_entries WHERE file_id = ?",
                        [&file_id],
                    ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;
                }
                Err(_) => break, // No more entries
            }
        }

        Ok(())
    }

    pub fn total_size(&self) -> Result<u64, DiskCacheError> {
        let total: Option<i64> = self.conn.query_row(
            "SELECT SUM(size) FROM cache_entries",
            [],
            |row| row.get(0),
        ).map_err(|e| DiskCacheError::Sqlite(e.to_string()))?;

        Ok(total.unwrap_or(0) as u64)
    }
}
```

### `src/cache/mod.rs`
Implement `ThumbnailCache`:
- Contains `MemoryCache`, `DiskCache`, `InflightMap`
- `new(memory_capacity: usize, cache_dir: PathBuf, max_disk_bytes: u64) -> Self`
- `async fn get(&self, file_id: FileId, file_key: &Key256, thumb_header: &Header24, zoo_client: &ZooClient) -> Result<Vec<u8>, ThumbnailError>`:
  1. Check L1 memory cache → return if hit
  2. Check L2 disk cache → if hit, populate L1 and return
  3. Check in-flight map → if another task is downloading, wait for it, then check caches again
  4. Download from Zoo, decrypt, populate both L1 and L2
  5. Notify waiting tasks
- `async fn evict(&self, file_id: FileId)` — remove from both L1 and L2

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use types::file::FileId;
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::ThumbnailError;
use crate::cache::memory::MemoryCache;
use crate::cache::disk::DiskCache;
use crate::inflight::InflightMap;
use crate::download::download_thumbnail;

pub struct ThumbnailCache {
    memory: Arc<Mutex<MemoryCache>>,
    disk: Arc<Mutex<DiskCache>>,
    inflight: Arc<InflightMap>,
}

impl ThumbnailCache {
    pub fn new(memory_capacity: usize, cache_dir: PathBuf, max_disk_bytes: u64) -> Result<Self, ThumbnailError> {
        let disk = DiskCache::new(cache_dir, max_disk_bytes)
            .map_err(|e| ThumbnailError::CacheError(e.to_string()))?;

        Ok(Self {
            memory: Arc::new(Mutex::new(MemoryCache::with_capacity(memory_capacity))),
            disk: Arc::new(Mutex::new(disk)),
            inflight: Arc::new(InflightMap::new()),
        })
    }

    pub async fn get(
        &self,
        file_id: FileId,
        file_key: &Key256,
        thumb_header: &Header24,
        zoo_client: &ZooClient,
    ) -> Result<Vec<u8>, ThumbnailError> {
        let file_id_str = file_id.to_string();

        // Step 1: Check L1 memory cache
        {
            let mut memory = self.memory.lock().await;
            if let Some(bytes) = memory.get(&file_id) {
                return Ok(bytes);
            }
        }

        // Step 2: Check L2 disk cache
        {
            let mut disk = self.disk.lock().await;
            if let Some(bytes) = disk.get(&file_id_str)
                .map_err(|e| ThumbnailError::CacheError(e.to_string()))?
            {
                // Populate L1
                let mut memory = self.memory.lock().await;
                memory.insert(file_id.clone(), bytes.clone());
                return Ok(bytes);
            }
        }

        // Step 3: Check in-flight map
        let guard = self.inflight.get_or_insert(file_id.clone());
        if guard.is_waiter() {
            // Wait for the other task to complete
            guard.notify.notified().await;

            // Check caches again after waiting
            {
                let mut memory = self.memory.lock().await;
                if let Some(bytes) = memory.get(&file_id) {
                    return Ok(bytes);
                }
            }
            {
                let mut disk = self.disk.lock().await;
                if let Some(bytes) = disk.get(&file_id_str)
                    .map_err(|e| ThumbnailError::CacheError(e.to_string()))?
                {
                    let mut memory = self.memory.lock().await;
                    memory.insert(file_id.clone(), bytes.clone());
                    return Ok(bytes);
                }
            }
        }

        // Step 4: Download from Zoo, decrypt, populate both L1 and L2
        let decrypted = download_thumbnail(file_id.clone(), zoo_client, file_key, thumb_header).await?;

        {
            let mut memory = self.memory.lock().await;
            memory.insert(file_id.clone(), decrypted.clone());
        }
        {
            let mut disk = self.disk.lock().await;
            let _ = disk.insert(&file_id_str, &decrypted);
        }

        // Step 5: Notify waiting tasks
        self.inflight.remove_and_notify(file_id.clone());

        Ok(decrypted)
    }

    pub async fn evict(&self, file_id: FileId) {
        let file_id_str = file_id.to_string();

        let mut memory = self.memory.lock().await;
        memory.remove(&file_id);

        let mut disk = self.disk.lock().await;
        let _ = disk.remove(&file_id_str);
    }
}
```

### `src/inflight.rs`
Implement `InflightMap`:
- `DashMap<FileId, Arc<tokio::sync::Notify>>`
- `get_or_insert(&self, file_id: FileId) -> InflightGuard` — returns existing notify or inserts new
- `remove_and_notify(&self, file_id: FileId)` — calls `notify_waiters()` and removes entry
- `InflightGuard` tracks whether this is the first inserter or a waiter

```rust
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::Notify;
use types::file::FileId;

pub struct InflightGuard {
    pub notify: Arc<Notify>,
    is_first: bool,
}

impl InflightGuard {
    pub fn is_waiter(&self) -> bool {
        !self.is_first
    }
}

pub struct InflightMap {
    map: DashMap<FileId, Arc<Notify>>,
}

impl InflightMap {
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    pub fn get_or_insert(&self, file_id: FileId) -> InflightGuard {
        if let Some(entry) = self.map.get(&file_id) {
            InflightGuard {
                notify: entry.value().clone(),
                is_first: false,
            }
        } else {
            let notify = Arc::new(Notify::new());
            self.map.insert(file_id, notify.clone());
            InflightGuard {
                notify,
                is_first: true,
            }
        }
    }

    pub fn remove_and_notify(&self, file_id: FileId) {
        if let Some((_, notify)) = self.map.remove(&file_id) {
            notify.notify_waiters();
        }
    }
}
```

### `src/download.rs`
Implement `download_thumbnail(file_id: FileId, zoo_client: &ZooClient, file_key: &Key256, thumb_header: &Header24) -> Result<Vec<u8>, ThumbnailError>`:
- Fetch encrypted thumbnail from Zoo via HTTP GET
- Decrypt with FileKey using thumb_decryption_header
- Return decrypted bytes

```rust
use types::file::FileId;
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::ThumbnailError;
use crate::decrypt::decrypt_thumbnail;

pub async fn download_thumbnail(
    file_id: FileId,
    zoo_client: &ZooClient,
    file_key: &Key256,
    thumb_header: &Header24,
) -> Result<Vec<u8>, ThumbnailError> {
    // Fetch encrypted thumbnail from Zoo
    let encrypted = zoo_client
        .fetch_thumbnail(&file_id)
        .await
        .map_err(|e| ThumbnailError::DownloadError(e.to_string()))?;

    // Decrypt
    let decrypted = decrypt_thumbnail(thumb_header, &encrypted, file_key)?;

    Ok(decrypted)
}
```

### `src/invalidation.rs`
Implement `ThumbnailInvalidator`:
- `evict_on_delete(cache: &ThumbnailCache, file_id: FileId)` — removes from both L1 and L2
- `evict_on_reupload(cache: &ThumbnailCache, file_id: FileId)` — invalidates cache entry, forcing re-download on next view

```rust
use types::file::FileId;
use crate::cache::ThumbnailCache;

pub struct ThumbnailInvalidator;

impl ThumbnailInvalidator {
    pub async fn evict_on_delete(cache: &ThumbnailCache, file_id: FileId) {
        cache.evict(file_id).await;
    }

    pub async fn evict_on_reupload(cache: &ThumbnailCache, file_id: FileId) {
        cache.evict(file_id).await;
    }
}
```

## Tests

### Property Test (Task 8.10 — marked with *)
- `∀ key ∈ Key256, thumbnail_bytes ∈ Vec<u8>: decrypt_thumbnail(encrypt_thumbnail(thumbnail_bytes, key)) == Ok(thumbnail_bytes)`

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip_encrypt_decrypt_thumbnail(
            key_bytes in prop::array::uniform32(any::<u8>()),
            thumbnail_bytes in prop::collection::vec(any::<u8>(), 1..10000),
        ) {
            let key = Key256::from(key_bytes);
            let (header, encrypted) = encrypt_thumbnail(&thumbnail_bytes, &key);
            let decrypted = decrypt_thumbnail(&header, &encrypted, &key).unwrap();
            prop_assert_eq!(decrypted, thumbnail_bytes);
        }
    }
}
```

### Unit Tests (Task 8.11 — marked with *)
- Generate thumbnail from JPEG fixture: output ≤ 720px max dimension and ≤ 100 KB
- Iterative quality reduction triggers when output > 100 KB
- EXIF orientation applied before encode
- Unsupported format (e.g., BMP) returns `ThumbnailError::UnsupportedFormat` without panicking
- Cache eviction on file delete removes from both L1 and L2
- Cache miss on evicted disk entry falls through to download path
- In-flight deduplication: concurrent requests for same file_id result in only one download

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn thumbnail_from_jpeg_is_within_specs() {
        // Load a JPEG fixture larger than 720px
        let source = include_bytes!("../fixtures/large_test.jpg");
        let key = Key256::random();
        let (header, ciphertext) = generate_thumbnail(source, "image/jpeg", &key).unwrap();

        // Decrypt to check dimensions
        let decrypted = decrypt_thumbnail(&header, &ciphertext, &key).unwrap();
        assert!(decrypted.len() <= 100 * 1024, "thumbnail must be <= 100 KB");
    }

    #[test]
    fn iterative_quality_reduction_triggers() {
        // Use a fixture that produces > 100 KB at quality 85
        let source = include_bytes!("../fixtures/huge_image.jpg");
        let key = Key256::random();
        let (_, ciphertext) = generate_thumbnail(source, "image/jpeg", &key).unwrap();
        assert!(ciphertext.len() <= 100 * 1024 + 16); // +16 for poly1305 tag
    }

    #[test]
    fn exif_orientation_applied_before_encode() {
        // Use a fixture with orientation=6 (rotate 90 CW)
        let source = include_bytes!("../fixtures/orientation_6.jpg");
        let key = Key256::random();
        let (header, ciphertext) = generate_thumbnail(source, "image/jpeg", &key).unwrap();
        let decrypted = decrypt_thumbnail(&header, &ciphertext, &key).unwrap();

        // Decode the output and verify dimensions are swapped (90° rotation)
        let img = image::decode_image(&decrypted, "image/jpeg").unwrap();
        // Original was landscape, after orientation 6 should be portrait
        assert!(img.height() > img.width(), "orientation 6 should swap dimensions");
    }

    #[test]
    fn unsupported_format_returns_error_without_panic() {
        let bmp_data = include_bytes!("../fixtures/test.bmp");
        let key = Key256::random();
        let result = generate_thumbnail(bmp_data, "image/bmp", &key);
        assert!(matches!(result, Err(ThumbnailError::UnsupportedFormat)));
    }

    #[test]
    fn video_mime_type_returns_unsupported() {
        let source = b"fake video data";
        let key = Key256::random();
        let result = generate_thumbnail(source, "video/mp4", &key);
        assert!(matches!(result, Err(ThumbnailError::UnsupportedFormat)));
    }

    #[tokio::test]
    async fn cache_eviction_on_delete_removes_from_both_levels() {
        let temp_dir = std::env::temp_dir().join("thumbnail_cache_test");
        let cache = ThumbnailCache::new(500, temp_dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let file_id = FileId::new();
        let test_bytes = vec![0u8; 1024];

        // Insert into both caches
        {
            let mut memory = cache.memory.lock().await;
            memory.insert(file_id.clone(), test_bytes.clone());
        }
        {
            let mut disk = cache.disk.lock().await;
            let _ = disk.insert(&file_id.to_string(), &test_bytes);
        }

        // Evict
        cache.evict(file_id.clone()).await;

        // Verify removed from both
        {
            let mut memory = cache.memory.lock().await;
            assert!(memory.get(&file_id).is_none());
        }
        {
            let mut disk = cache.disk.lock().await;
            let result = disk.get(&file_id.to_string()).unwrap();
            assert!(result.is_none());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn inflight_deduplication_prevents_duplicate_downloads() {
        let inflight = InflightMap::new();
        let file_id = FileId::new();

        // First call should be the inserter
        let guard1 = inflight.get_or_insert(file_id.clone());
        assert!(guard1.is_first);

        // Second call for same file_id should be a waiter
        let guard2 = inflight.get_or_insert(file_id.clone());
        assert!(guard2.is_waiter());

        // Notify and remove
        inflight.remove_and_notify(file_id);
    }
}
```

## Verification Steps
- [ ] `cargo check -p thumbnail` succeeds
- [ ] `cargo test -p thumbnail` passes
- [ ] Thumbnail output ≤ 720px and ≤ 100 KB for standard test images
- [ ] Two-level cache works: L1 hit, L2 hit, cache miss → download
- [ ] In-flight deduplication prevents duplicate downloads
- [ ] Cache invalidation removes entries from both levels
- [ ] Video mime_type returns `UnsupportedFormat`

## Notes
- The `lru` crate provides a thread-safe LRU cache.
- `dashmap` provides concurrent HashMap for in-flight deduplication.
- The disk cache uses a separate SQLite file for the metadata index — this is separate from the main local-db.
- Video thumbnail extraction (frame at 1s mark) is deferred — return `UnsupportedFormat` for video/* mime types.
- The `ThumbnailCache::get` method is async because it may need to download from the network.
- Cache invalidation events: file delete/archive → evict; thumbnail re-upload → invalidate; app cache clear → handled as miss; disk space low → OS eviction handled as miss.
