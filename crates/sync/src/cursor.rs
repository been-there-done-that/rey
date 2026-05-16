use crate::error::SyncError;
use local_db::sync_state;
use local_db::LocalDb;

pub fn read_cursor(db: &LocalDb, key: &str) -> Result<Option<i64>, SyncError> {
    sync_state::read_cursor(&db.conn, key).map_err(SyncError::DbError)
}

pub fn write_cursor(db: &LocalDb, key: &str, value: i64) -> Result<(), SyncError> {
    sync_state::write_cursor(&db.conn, key, value).map_err(SyncError::DbError)
}
