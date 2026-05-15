use crate::commands::error::CommandError;
use crate::state::AppState;
use local_db::search;
use types::file::FileRecord;

pub async fn search_files(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<FileRecord>, CommandError> {
    let db = state.db.lock().await;
    let results =
        search::search_text(&db.conn, &query).map_err(CommandError::DbError)?;
    Ok(results)
}

pub async fn search_by_date(
    state: tauri::State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<FileRecord>, CommandError> {
    let db = state.db.lock().await;
    let results =
        search::search_by_date(&db.conn, start_ms, end_ms).map_err(CommandError::DbError)?;
    Ok(results)
}

pub async fn search_by_location(
    state: tauri::State<'_, AppState>,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
) -> Result<Vec<FileRecord>, CommandError> {
    let db = state.db.lock().await;
    let results = search::search_by_location(&db.conn, lat_min, lat_max, lon_min, lon_max)
        .map_err(CommandError::DbError)?;
    Ok(results)
}
