use crate::db::files::list_files_for_sync;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use types::error::{ApiError, ErrorCode, ErrorResponse};
use types::file::EncryptedFileRecord;
use types::sync::SyncFilesResponse;
use uuid::Uuid;

pub async fn sync_files(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Query(query): axum::extract::Query<SyncQueryParams>,
) -> Result<Json<SyncFilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.unwrap_or(100);

    let since = query.since.map(|ts| {
        chrono::DateTime::from_timestamp_millis(ts)
            .unwrap_or(chrono::DateTime::from_timestamp(0, 0).unwrap())
    });

    let files = list_files_for_sync(&state.pool, user_id, since, limit)
        .await
        .map_err(internal_error)?;

    let latest_updated_at = files
        .last()
        .map(|f| f.updation_time.timestamp_millis())
        .unwrap_or(0);

    let updated_files: Vec<EncryptedFileRecord> = files
        .into_iter()
        .map(|f| EncryptedFileRecord {
            id: f.id,
            collection_id: f.collection_id,
            cipher: f.cipher,
            encrypted_key: f.encrypted_key,
            key_decryption_nonce: f.key_decryption_nonce,
            file_decryption_header: f.file_decryption_header,
            thumb_decryption_header: f.thumb_decryption_header,
            encrypted_metadata: f.encrypted_metadata,
            encrypted_thumbnail: f.encrypted_thumbnail,
            thumbnail_size: f.thumbnail_size,
            file_size: f.file_size,
            mime_type: f.mime_type,
            content_hash: f.content_hash,
            object_key: f.object_key,
            updation_time: f.updation_time.timestamp_millis(),
            created_at: f.created_at.timestamp_millis(),
            archived_at: f.archived_at.map(|t| t.timestamp_millis()),
        })
        .collect();

    let has_more = updated_files.len() >= limit as usize;

    Ok(Json(SyncFilesResponse {
        updated_files,
        deleted_file_ids: Vec::new(),
        has_more,
        latest_updated_at,
    }))
}

#[derive(Debug, serde::Deserialize)]
pub struct SyncQueryParams {
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

fn internal_error(e: crate::error::ZooError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: ApiError {
                code: ErrorCode::InternalError,
                message: e.to_string(),
                details: None,
            },
        }),
    )
}
