use crate::commands::error::CommandError;
use crate::state::AppState;
use local_db::collections;
use types::collection::Collection;

pub async fn list_collections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Collection>, CommandError> {
    let db = state.db.lock().await;
    let cols = collections::list_collections(&db.conn).map_err(CommandError::DbError)?;
    Ok(cols)
}

pub async fn create_collection(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Collection, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    let record = Collection {
        id: id.clone(),
        name,
        encrypted_key: String::new(),
        key_nonce: String::new(),
        updation_time: chrono::Utc::now().timestamp_millis(),
        created_at: chrono::Utc::now().timestamp_millis(),
        archived_at: None,
    };

    let db = state.db.lock().await;
    collections::upsert_collection(&db.conn, &record).map_err(CommandError::DbError)?;
    Ok(record)
}

pub async fn archive_collection(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    let db = state.db.lock().await;
    collections::archive_collection(&db.conn, &id).map_err(CommandError::DbError)?;
    Ok(())
}
