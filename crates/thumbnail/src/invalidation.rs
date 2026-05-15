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
