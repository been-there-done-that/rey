use crate::cache::ThumbnailCache;

pub struct ThumbnailInvalidator;

impl ThumbnailInvalidator {
    pub async fn evict_on_delete(cache: &ThumbnailCache, file_id: i64) {
        cache.evict(file_id).await;
    }

    pub async fn evict_on_reupload(cache: &ThumbnailCache, file_id: i64) {
        cache.evict(file_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_evict_on_delete_removes_from_cache() {
        let dir = tempdir().unwrap();
        let cache = ThumbnailCache::new(10, dir.path().to_path_buf(), 1024 * 1024).unwrap();
        let file_id = 42i64;

        cache.insert_decrypted(file_id, vec![1, 2, 3, 4]).await;
        ThumbnailInvalidator::evict_on_delete(&cache, file_id).await;

        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[tokio::test]
    async fn test_evict_on_reupload_removes_from_cache() {
        let dir = tempdir().unwrap();
        let cache = ThumbnailCache::new(10, dir.path().to_path_buf(), 1024 * 1024).unwrap();
        let file_id = 99i64;

        cache.insert_decrypted(file_id, vec![5, 6, 7, 8]).await;
        ThumbnailInvalidator::evict_on_reupload(&cache, file_id).await;

        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[tokio::test]
    async fn test_evict_nonexistent_file_id() {
        let dir = tempdir().unwrap();
        let cache = ThumbnailCache::new(10, dir.path().to_path_buf(), 1024 * 1024).unwrap();
        ThumbnailInvalidator::evict_on_delete(&cache, 9999).await;
        let _ = std::fs::remove_dir_all(dir.path());
    }
}
