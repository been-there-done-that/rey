use crate::config::DownloadMode;
use crate::db::files::{archive_file, get_file_for_download, list_files_for_user};
use crate::s3::presigner::presign_download;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use types::error::{ApiError, ErrorCode, ErrorResponse};
use uuid::Uuid;

pub async fn get_download_url(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(file_id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let file = get_file_for_download(&state.pool, user_id, file_id)
        .await
        .map_err(internal_error)?;

    let file = match file {
        Some(f) => f,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "file not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    match &state.config.download_mode {
        DownloadMode::Redirect { presigned_ttl } => {
            let url = presign_download(
                &state.s3_client,
                &state.config.s3_bucket,
                &file.object_key,
                *presigned_ttl,
            )
            .await
            .map_err(internal_error)?;

            Ok(Json(serde_json::json!({
                "url": url,
                "file_id": file.id,
                "content_hash": file.content_hash,
                "file_size": file.file_size,
                "mime_type": file.mime_type,
            })))
        }
        DownloadMode::Proxy { .. } => Ok(Json(serde_json::json!({
            "proxy": true,
            "file_id": file.id,
            "content_hash": file.content_hash,
            "file_size": file.file_size,
            "mime_type": file.mime_type,
        }))),
    }
}

pub async fn archive(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(file_id): axum::extract::Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    archive_file(&state.pool, user_id, file_id)
        .await
        .map_err(|e| match e {
            crate::error::ZooError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "file not found or already archived".to_string(),
                        details: None,
                    },
                }),
            ),
            _ => internal_error(e),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Result<Json<Vec<FileItem>>, (StatusCode, Json<ErrorResponse>)> {
    let since = query.since_time.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let files = list_files_for_user(&state.pool, user_id, since, limit)
        .await
        .map_err(internal_error)?;

    let items = files
        .into_iter()
        .map(|f| FileItem {
            id: f.id,
            collection_id: f.collection_id,
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
            created_at: f.created_at.timestamp_millis(),
            updation_time: f.updation_time.timestamp_millis(),
            archived_at: f.archived_at.map(|t| t.timestamp_millis()),
        })
        .collect();

    Ok(Json(items))
}

#[derive(Debug, serde::Deserialize)]
pub struct ListQuery {
    pub collection_id: Option<String>,
    pub since_time: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct FileItem {
    pub id: i64,
    pub collection_id: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub encrypted_metadata: String,
    pub encrypted_thumbnail: Option<String>,
    pub thumbnail_size: Option<i32>,
    pub file_size: i64,
    pub mime_type: String,
    pub content_hash: String,
    pub created_at: i64,
    pub updation_time: i64,
    pub archived_at: Option<i64>,
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
