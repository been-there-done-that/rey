use crate::commands::error::CommandError;
use crate::state::AppState;

pub async fn trigger_sync(_state: tauri::State<'_, AppState>) -> Result<(), CommandError> {
    Ok(())
}

pub async fn get_sync_status(
    _state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, CommandError> {
    Ok(serde_json::json!({
        "last_sync": null,
        "in_progress": false,
    }))
}
