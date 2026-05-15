use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct CacheIndex {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    last_accessed: i64,
}

#[derive(Debug)]
pub struct DiskCacheError(String);

impl std::fmt::Display for DiskCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DiskCacheError {
    pub fn new(msg: String) -> Self {
        Self(msg)
    }
}

pub struct DiskCache {
    cache_dir: PathBuf,
    max_bytes: u64,
    index: CacheIndex,
}

impl DiskCache {
    pub fn new(cache_dir: PathBuf, max_bytes: u64) -> Result<Self, DiskCacheError> {
        let thumb_dir = cache_dir.join("thumbnails");
        fs::create_dir_all(&thumb_dir).map_err(|e| DiskCacheError::new(e.to_string()))?;

        let index_path = cache_dir.join("thumbnail_index.json");
        let index = if index_path.exists() {
            let content = fs::read_to_string(&index_path)
                .map_err(|e| DiskCacheError::new(e.to_string()))?;
            serde_json::from_str(&content).unwrap_or_else(|_| CacheIndex {
                entries: HashMap::new(),
            })
        } else {
            CacheIndex {
                entries: HashMap::new(),
            }
        };

        Ok(Self {
            cache_dir,
            max_bytes,
            index,
        })
    }

    fn save_index(&self) -> Result<(), DiskCacheError> {
        let index_path = self.cache_dir.join("thumbnail_index.json");
        let content = serde_json::to_string(&self.index)
            .map_err(|e| DiskCacheError::new(e.to_string()))?;
        fs::write(&index_path, content)
            .map_err(|e| DiskCacheError::new(e.to_string()))
    }

    fn thumb_path(&self, file_id: &str) -> PathBuf {
        self.cache_dir.join("thumbnails").join(file_id)
    }

    pub fn get(&mut self, file_id: &str) -> Result<Option<Vec<u8>>, DiskCacheError> {
        let path = self.thumb_path(file_id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&path).map_err(|e| DiskCacheError::new(e.to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        if let Some(entry) = self.index.entries.get_mut(file_id) {
            entry.last_accessed = now;
        }
        self.save_index()?;

        Ok(Some(bytes))
    }

    pub fn insert(&mut self, file_id: &str, bytes: &[u8]) -> Result<(), DiskCacheError> {
        let path = self.thumb_path(file_id);
        fs::write(&path, bytes).map_err(|e| DiskCacheError::new(e.to_string()))?;

        let size = bytes.len() as u64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.index.entries.insert(
            file_id.to_string(),
            CacheEntry {
                size,
                last_accessed: now,
            },
        );
        self.save_index()?;

        self.evict_lru_until_below(self.max_bytes)?;

        Ok(())
    }

    pub fn remove(&mut self, file_id: &str) -> Result<(), DiskCacheError> {
        let path = self.thumb_path(file_id);
        let _ = fs::remove_file(&path);
        self.index.entries.remove(file_id);
        self.save_index()
    }

    pub fn evict_lru_until_below(&mut self, limit_bytes: u64) -> Result<(), DiskCacheError> {
        loop {
            let total = self.total_size()?;
            if total <= limit_bytes {
                break;
            }

            let lru_entry = self
                .index
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(k, _)| k.clone());

            match lru_entry {
                Some(file_id) => {
                    let path = self.thumb_path(&file_id);
                    let _ = fs::remove_file(&path);
                    self.index.entries.remove(&file_id);
                }
                None => break,
            }
        }

        self.save_index()
    }

    pub fn total_size(&self) -> Result<u64, DiskCacheError> {
        let total: u64 = self.index.entries.values().map(|e| e.size).sum();
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("thumb_disk_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_disk_cache_insert_and_get() {
        let dir = temp_dir();
        let mut cache = DiskCache::new(dir.clone(), 1024 * 1024).unwrap();
        cache.insert("file1", &[1, 2, 3]).unwrap();
        let result = cache.get("file1").unwrap();
        assert_eq!(result, Some(vec![1, 2, 3]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_disk_cache_remove() {
        let dir = temp_dir();
        let mut cache = DiskCache::new(dir.clone(), 1024 * 1024).unwrap();
        cache.insert("file1", &[1, 2, 3]).unwrap();
        cache.remove("file1").unwrap();
        let result = cache.get("file1").unwrap();
        assert!(result.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_disk_cache_eviction() {
        let dir = temp_dir();
        let mut cache = DiskCache::new(dir.clone(), 100).unwrap();
        cache.insert("file1", &[0u8; 50]).unwrap();
        cache.insert("file2", &[0u8; 50]).unwrap();
        cache.insert("file3", &[0u8; 50]).unwrap(); // should trigger eviction
        let total = cache.total_size().unwrap();
        assert!(total <= 100);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_disk_cache_total_size() {
        let dir = temp_dir();
        let mut cache = DiskCache::new(dir.clone(), 1024 * 1024).unwrap();
        cache.insert("file1", &[0u8; 100]).unwrap();
        cache.insert("file2", &[0u8; 200]).unwrap();
        assert_eq!(cache.total_size().unwrap(), 300);
        let _ = fs::remove_dir_all(&dir);
    }
}
