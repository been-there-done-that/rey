use crate::commands::error::CommandError;
use crate::state::AppState;
use local_db::files;
use types::file::FileRecord;

pub async fn list_files(
    state: tauri::State<'_, AppState>,
    collection_id: String,
) -> Result<Vec<FileRecord>, CommandError> {
    let db = state.db.lock().await;
    let files_list =
        files::list_files(&db.conn, &collection_id).map_err(CommandError::DbError)?;
    Ok(files_list)
}

pub async fn get_file(
    state: tauri::State<'_, AppState>,
    file_id: i64,
) -> Result<Option<FileRecord>, CommandError> {
    let db = state.db.lock().await;
    let file = files::get_file(&db.conn, file_id).map_err(CommandError::DbError)?;
    Ok(file)
}

pub async fn archive_file(
    state: tauri::State<'_, AppState>,
    file_id: i64,
) -> Result<(), CommandError> {
    let db = state.db.lock().await;
    files::archive_files(&db.conn, &[file_id]).map_err(CommandError::DbError)?;
    Ok(())
}

pub async fn download_file(
    state: tauri::State<'_, AppState>,
    file_id: i64,
    destination: String,
) -> Result<(), CommandError> {
    let bytes = state.zoo_client.download_file(file_id).await?;
    std::fs::write(&destination, bytes).map_err(CommandError::Io)?;
    Ok(())
}
