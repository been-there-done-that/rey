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

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("thumb_cache_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
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
}
