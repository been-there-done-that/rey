use crate::commands::error::CommandError;
use crate::state::AppState;

pub async fn get_thumbnail(
    _state: tauri::State<'_, AppState>,
    _file_id: i64,
) -> Result<Option<Vec<u8>>, CommandError> {
    Ok(None)
}

pub async fn evict_thumbnail(
    _state: tauri::State<'_, AppState>,
    _file_id: i64,
) -> Result<(), CommandError> {
    Ok(())
}
