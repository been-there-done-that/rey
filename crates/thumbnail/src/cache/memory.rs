use lru::LruCache;
use std::num::NonZeroUsize;

const DEFAULT_CAPACITY: usize = 500;

pub struct MemoryCache {
    cache: LruCache<i64, Vec<u8>>,
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

    pub fn get(&mut self, file_id: &i64) -> Option<Vec<u8>> {
        self.cache.get(file_id).cloned()
    }

    pub fn insert(&mut self, file_id: i64, bytes: Vec<u8>) {
        self.cache.put(file_id, bytes);
    }

    pub fn remove(&mut self, file_id: &i64) {
        self.cache.pop(file_id);
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_cache_insert_and_get() {
        let mut cache = MemoryCache::new();
        cache.insert(1, vec![1, 2, 3]);
        assert_eq!(cache.get(&1), Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_memory_cache_remove() {
        let mut cache = MemoryCache::new();
        cache.insert(1, vec![1, 2, 3]);
        cache.remove(&1);
        assert!(cache.get(&1).is_none());
    }

    #[test]
    fn test_memory_cache_evicts_lru() {
        let mut cache = MemoryCache::with_capacity(2);
        cache.insert(1, vec![1]);
        cache.insert(2, vec![2]);
        cache.insert(3, vec![3]); // should evict key 1
        assert!(cache.get(&1).is_none());
        assert!(cache.get(&2).is_some());
        assert!(cache.get(&3).is_some());
    }

    #[test]
    fn test_memory_cache_len() {
        let mut cache = MemoryCache::new();
        assert_eq!(cache.len(), 0);
        cache.insert(1, vec![1]);
        cache.insert(2, vec![2]);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_memory_cache_is_empty() {
        let mut cache = MemoryCache::new();
        assert!(cache.is_empty());
        cache.insert(1, vec![1]);
        assert!(!cache.is_empty());
    }

    #[test]
    fn test_memory_cache_default() {
        let cache = MemoryCache::default();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_memory_cache_get_nonexistent() {
        let mut cache = MemoryCache::new();
        assert!(cache.get(&999).is_none());
    }

    #[test]
    fn test_memory_cache_overwrite() {
        let mut cache = MemoryCache::with_capacity(10);
        cache.insert(1, vec![1, 2, 3]);
        cache.insert(1, vec![4, 5, 6]);
        assert_eq!(cache.get(&1), Some(vec![4, 5, 6]));
        assert_eq!(cache.len(), 1);
    }
}
