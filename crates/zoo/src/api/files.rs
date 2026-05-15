use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use types::error::{ApiError, ErrorCode, ErrorResponse};
use uuid::Uuid;
use crate::db::files::{archive_file, get_file_for_download};
use crate::s3::presigner::presign_download;
use crate::state::AppState;
use crate::config::DownloadMode;

pub async fn get_download_url(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(file_id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let file = get_file_for_download(&state.pool, user_id, file_id)
        .await
        .map_err(|e| internal_error(e))?;

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
            let url = presign_download(&state.s3_client, &state.config.s3_bucket, &file.object_key, *presigned_ttl)
                .await
                .map_err(|e| internal_error(e))?;

            Ok(Json(serde_json::json!({
                "url": url,
                "file_id": file.id,
                "content_hash": file.content_hash,
                "file_size": file.file_size,
                "mime_type": file.mime_type,
            })))
        }
        DownloadMode::Proxy { .. } => {
            Ok(Json(serde_json::json!({
                "proxy": true,
                "file_id": file.id,
                "content_hash": file.content_hash,
                "file_size": file.file_size,
                "mime_type": file.mime_type,
            })))
        }
    }
}

pub async fn archive(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(file_id): axum::extract::Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    archive_file(&state.pool, user_id, file_id)
        .await
        .map_err(|e| internal_error(e))?;
    Ok(StatusCode::NO_CONTENT)
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
