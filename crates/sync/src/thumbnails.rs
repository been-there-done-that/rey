use crate::error::SyncError;
use local_db::files;
use local_db::LocalDb;
use thumbnail::cache::ThumbnailCache;

pub async fn queue_new_files(db: &LocalDb, _cache: &ThumbnailCache) -> Result<(), SyncError> {
    let _files = files::list_files_without_thumbnail(&db.conn).map_err(SyncError::DbError)?;

    Ok(())
}
