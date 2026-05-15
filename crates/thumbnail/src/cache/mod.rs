pub mod disk;
pub mod memory;

use crate::cache::disk::DiskCache;
use crate::cache::memory::MemoryCache;
use crate::error::ThumbnailError;
use crate::inflight::InflightMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use types::crypto::{Header24, Key256};

pub struct ThumbnailCache {
    memory: Arc<Mutex<MemoryCache>>,
    disk: Arc<Mutex<DiskCache>>,
    inflight: Arc<InflightMap>,
}

impl ThumbnailCache {
    pub fn new(
        memory_capacity: usize,
        cache_dir: PathBuf,
        max_disk_bytes: u64,
    ) -> Result<Self, ThumbnailError> {
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
        file_id: i64,
        file_key: &Key256,
        thumb_header: &Header24,
        fetcher: impl FnOnce() -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<u8>, ThumbnailError>> + Send>,
            > + Send,
    ) -> Result<Vec<u8>, ThumbnailError> {
        let file_id_str = file_id.to_string();

        {
            let mut memory = self.memory.lock().await;
            if let Some(bytes) = memory.get(&file_id) {
                return Ok(bytes);
            }
        }

        {
            let mut disk = self.disk.lock().await;
            if let Some(bytes) = disk
                .get(&file_id_str)
                .map_err(|e| ThumbnailError::CacheError(e.to_string()))?
            {
                let mut memory = self.memory.lock().await;
                memory.insert(file_id, bytes.clone());
                return Ok(bytes);
            }
        }

        let guard = self.inflight.get_or_insert(file_id);
        if guard.is_waiter() {
            guard.notify.notified().await;

            {
                let mut memory = self.memory.lock().await;
                if let Some(bytes) = memory.get(&file_id) {
                    return Ok(bytes);
                }
            }
            {
                let mut disk = self.disk.lock().await;
                if let Some(bytes) = disk
                    .get(&file_id_str)
                    .map_err(|e| ThumbnailError::CacheError(e.to_string()))?
                {
                    let mut memory = self.memory.lock().await;
                    memory.insert(file_id, bytes.clone());
                    return Ok(bytes);
                }
            }
        }

        let decrypted = fetcher().await?;

        {
            let mut memory = self.memory.lock().await;
            memory.insert(file_id, decrypted.clone());
        }
        {
            let mut disk = self.disk.lock().await;
            let _ = disk.insert(&file_id_str, &decrypted);
        }

        self.inflight.remove_and_notify(file_id);

        // Decrypt if needed (the fetcher returns encrypted data)
        let decrypted = crypto::stream_decrypt(thumb_header, &decrypted, file_key)
            .map_err(ThumbnailError::Crypto)?;

        Ok(decrypted)
    }

    pub async fn evict(&self, file_id: i64) {
        let file_id_str = file_id.to_string();

        let mut memory = self.memory.lock().await;
        memory.remove(&file_id);

        let mut disk = self.disk.lock().await;
        let _ = disk.remove(&file_id_str);
    }

    pub async fn insert_decrypted(&self, file_id: i64, bytes: Vec<u8>) {
        let file_id_str = file_id.to_string();

        {
            let mut memory = self.memory.lock().await;
            memory.insert(file_id, bytes.clone());
        }
        {
            let mut disk = self.disk.lock().await;
            let _ = disk.insert(&file_id_str, &bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::crypto::Key256;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thumb_cache_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn make_test_pair() -> (Header24, Vec<u8>, Key256) {
        let key = Key256::new([42u8; 32]);
        let plaintext = b"test thumbnail data";
        let (header, ciphertext) = crypto::stream_encrypt(plaintext, &key);
        (header, ciphertext, key)
    }

    #[tokio::test]
    async fn test_cache_eviction_removes_from_both_levels() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        cache.insert_decrypted(1, vec![1, 2, 3]).await;

        {
            let mut memory = cache.memory.lock().await;
            assert!(memory.get(&1).is_some());
        }

        cache.evict(1).await;

        {
            let mut memory = cache.memory.lock().await;
            assert!(memory.get(&1).is_none());
        }
        {
            let mut disk = cache.disk.lock().await;
            let result = disk.get("1").unwrap();
            assert!(result.is_none());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_get_memory_hit() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let expected = vec![10, 20, 30];
        cache.insert_decrypted(42, expected.clone()).await;

        let (header, _ciphertext, key) = make_test_pair();
        let mut fetcher_called = false;
        let result = cache
            .get(42, &key, &header, || {
                fetcher_called = true;
                Box::pin(async { Ok(vec![]) })
            })
            .await
            .unwrap();

        assert_eq!(result, expected);
        assert!(!fetcher_called);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_get_disk_hit_populates_memory() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let expected = vec![40, 50, 60];
        cache.insert_decrypted(99, expected.clone()).await;

        // Evict from memory only by creating a new cache with empty memory but same disk
        let cache2 = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let (header, _ciphertext, key) = make_test_pair();
        let mut fetcher_called = false;
        let result = cache2
            .get(99, &key, &header, || {
                fetcher_called = true;
                Box::pin(async { Ok(vec![]) })
            })
            .await
            .unwrap();

        assert_eq!(result, expected);
        assert!(!fetcher_called);

        // Verify memory was repopulated
        {
            let mut memory = cache2.memory.lock().await;
            assert!(memory.get(&99).is_some());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_get_fetcher_path() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let (header, ciphertext, key) = make_test_pair();
        let fetcher_data = ciphertext.clone();
        let result = cache
            .get(77, &key, &header, || {
                let data = fetcher_data.clone();
                Box::pin(async move { Ok(data) })
            })
            .await
            .unwrap();

        assert_eq!(result, b"test thumbnail data");

        // Verify stored in memory
        {
            let mut memory = cache.memory.lock().await;
            assert!(memory.get(&77).is_some());
        }

        // Verify stored on disk
        {
            let mut disk = cache.disk.lock().await;
            let disk_result = disk.get("77").unwrap();
            assert!(disk_result.is_some());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_get_fetcher_error_propagates() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let (header, _ciphertext, key) = make_test_pair();
        let result = cache
            .get(55, &key, &header, || {
                Box::pin(async { Err(ThumbnailError::NotFound) })
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ThumbnailError::NotFound));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_get_wrong_key_decrypt_fails() {
        let dir = temp_dir();
        let cache = ThumbnailCache::new(500, dir.clone(), 2 * 1024 * 1024 * 1024).unwrap();

        let correct_key = Key256::new([42u8; 32]);
        let (header, ciphertext) = crypto::stream_encrypt(b"secret data", &correct_key);

        let wrong_key = Key256::new([99u8; 32]);
        let result = cache
            .get(33, &wrong_key, &header, || {
                let data = ciphertext.clone();
                Box::pin(async move { Ok(data) })
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ThumbnailError::Crypto(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_cache_new_with_invalid_dir_fails() {
        let invalid_dir = if cfg!(windows) {
            PathBuf::from("Z:\\\\nonexistent\\\\drive\\\\path")
        } else {
            PathBuf::from("/dev/null/invalid/cache")
        };
        let result = ThumbnailCache::new(500, invalid_dir, 1024 * 1024);
        assert!(result.is_err());
    }
}
