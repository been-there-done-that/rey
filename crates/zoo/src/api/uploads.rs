use crate::db::files::insert_file_record;
use crate::db::upload_parts::{insert_parts_batch, mark_part_uploaded};
use crate::db::uploads::{
    create_upload, get_upload, patch_upload_status, update_bitmask, update_heartbeat,
    update_s3_info,
};
use crate::error::ZooError;
use crate::s3::presigner::{build_complete_url, presign_part_upload};
use crate::state::{validate_transition, AppState};
use crate::types::{RegisterRequest, UploadRequest};
use crate::validation::{
    validate_file_size, validate_part_count, validate_part_md5s, validate_part_size,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use types::error::{ApiError, ErrorCode, ErrorResponse};
use types::upload::{UploadState, UploadStatus};
use uuid::Uuid;

pub async fn create(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    headers: axum::http::HeaderMap,
    Json(req): Json<UploadRequest>,
) -> Result<(StatusCode, Json<UploadState>), (StatusCode, Json<ErrorResponse>)> {
    let device_id_str = headers
        .get("x-device-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| validation_error("missing x-device-id header".to_string()))?;

    let device_id = Uuid::parse_str(device_id_str)
        .map_err(|_| validation_error("invalid device id format".to_string()))?;

    validate_file_size(req.file_size as u64).map_err(|e| validation_error(e.to_string()))?;
    validate_part_size(req.part_size as u64).map_err(|e| validation_error(e.to_string()))?;
    validate_part_count(req.part_count).map_err(|e| validation_error(e.to_string()))?;
    validate_part_md5s(&req.part_md5s, req.part_count)
        .map_err(|e| validation_error(e.to_string()))?;

    let expires_at = Utc::now() + chrono::Duration::hours(24);
    let object_key = format!("{}/{}", user_id, Uuid::new_v4());

    let upload_id = create_upload(
        &state.pool,
        user_id,
        device_id,
        &req.file_hash,
        req.file_size,
        req.mime_type.as_deref(),
        req.part_size,
        req.part_count as i16,
        expires_at,
        &object_key,
    )
    .await
    .map_err(|e| match e {
        ZooError::UploadAlreadyExists => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::UploadAlreadyExists,
                    message: "upload already exists for this file hash".to_string(),
                    details: None,
                },
            }),
        ),
        _ => internal_error(e),
    })?;

    let parts: Vec<(i16, i32, String)> = req
        .part_md5s
        .iter()
        .enumerate()
        .map(|(i, md5)| (i as i16 + 1, req.part_size, md5.clone()))
        .collect();

    insert_parts_batch(&state.pool, upload_id, &parts)
        .await
        .map_err(internal_error)?;

    let presigned_urls = generate_presigned_urls(
        &state.s3_client,
        &state.config.s3_bucket,
        &object_key,
        &req.part_md5s,
        state.config.presigned_ttl,
    )
    .await
    .map_err(internal_error)?;

    let complete_url = build_complete_url(&state.config.s3_bucket,
        &object_key,
        &presigned_urls.upload_id_s3,
    );

    let urls_expire_at =
        Utc::now() + chrono::Duration::from_std(state.config.presigned_ttl).unwrap();

    update_s3_info(
        &state.pool,
        upload_id,
        &presigned_urls.upload_id_s3,
        &complete_url,
        urls_expire_at,
    )
    .await
    .map_err(internal_error)?;

    patch_upload_status(&state.pool, upload_id, "pending")
        .await
        .map_err(internal_error)?;

    let bitmask_bytes = vec![0u8; (req.part_count as usize).div_ceil(8)];
    update_bitmask(&state.pool, upload_id, &bitmask_bytes)
        .await
        .map_err(internal_error)?;

    let upload_state = get_upload_state(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    Ok((StatusCode::CREATED, Json(upload_state)))
}

pub async fn get_status(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
) -> Result<Json<UploadState>, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    match upload {
        Some(u) if u.user_id == user_id => {
            let upload_state = upload_to_state(u);
            Ok(Json(upload_state))
        }
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::Forbidden,
                    message: "not your upload".to_string(),
                    details: None,
                },
            }),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::NotFound,
                    message: "upload not found".to_string(),
                    details: None,
                },
            }),
        )),
    }
}

pub async fn heartbeat(
    State(state): State<AppState>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    update_heartbeat(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn confirm_part(
    State(state): State<AppState>,
    axum::extract::Path((upload_id, part_number)): axum::extract::Path<(Uuid, i16)>,
    Json(body): Json<PartConfirmRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    mark_part_uploaded(&state.pool, upload_id, part_number, &body.etag)
        .await
        .map_err(internal_error)?;

    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    if let Some(u) = upload {
        let mut bitmask = u.parts_bitmask.unwrap_or_default();
        let idx = (part_number - 1) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;

        if byte_idx < bitmask.len() {
            bitmask[byte_idx] |= 1 << bit_idx;
        }

        update_bitmask(&state.pool, upload_id, &bitmask)
            .await
            .map_err(internal_error)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn complete(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
) -> Result<Json<UploadState>, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let current_status = parse_status(&upload.status);
    validate_transition(current_status, UploadStatus::S3Completed).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::InvalidStateTransition,
                    message: e.message,
                    details: None,
                },
            }),
        )
    })?;

    patch_upload_status(&state.pool, upload_id, "s3_completed")
        .await
        .map_err(internal_error)?;

    let upload_state = get_upload_state(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(upload_state))
}

pub async fn register_file(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let current_status = parse_status(&upload.status);
    validate_transition(current_status, UploadStatus::Registering).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::InvalidStateTransition,
                    message: e.message,
                    details: None,
                },
            }),
        )
    })?;

    let object_key = upload.object_key.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::ValidationError,
                    message: "upload has no object key".to_string(),
                    details: None,
                },
            }),
        )
    })?;

    let file_id = insert_file_record(
        &state.pool,
        user_id,
        &req.collection_id,
        &req.encrypted_key,
        &req.key_decryption_nonce,
        &req.file_decryption_header,
        req.thumb_decryption_header.as_deref(),
        &req.encrypted_metadata,
        req.encrypted_thumbnail.as_deref(),
        req.thumbnail_size,
        upload.file_size,
        upload
            .mime_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
        &upload.file_hash,
        &object_key,
    )
    .await
    .map_err(internal_error)?;

    patch_upload_status(&state.pool, upload_id, "done")
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "file_id": file_id,
        "upload_id": upload_id.to_string(),
    })))
}

pub async fn fail(
    State(state): State<AppState>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
    Json(_body): Json<FailRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    patch_upload_status(&state.pool, upload_id, "failed")
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize)]
pub struct PatchStatusRequest {
    pub status: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct PresignRequest {
    pub part_md5s: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PresignResponse {
    pub urls: Vec<String>,
    pub complete_url: String,
}

pub async fn patch_status(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
    Json(req): Json<PatchStatusRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let current_status = parse_status(&upload.status);
    let target_status = parse_status(&req.status);
    validate_transition(current_status, target_status).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::InvalidStateTransition,
                    message: e.message,
                    details: None,
                },
            }),
        )
    })?;

    patch_upload_status(&state.pool, upload_id, &req.status)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

pub async fn presign(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
    Json(req): Json<PresignRequest>,
) -> Result<Json<PresignResponse>, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let object_key = upload.object_key.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::ValidationError,
                    message: "upload has no object key".to_string(),
                    details: None,
                },
            }),
        )
    })?;

    let presigned_urls = generate_presigned_urls(
        &state.s3_client,
        &state.config.s3_bucket,
        &object_key,
        &req.part_md5s,
        state.config.presigned_ttl,
    )
    .await
    .map_err(internal_error)?;

    let urls: Vec<String> = presigned_urls.urls.clone();
    let complete_url = build_complete_url(&state.config.s3_bucket,
        &object_key,
        &presigned_urls.upload_id_s3,
    );

    let urls_expire_at =
        Utc::now() + chrono::Duration::from_std(state.config.presigned_ttl).unwrap();

    update_s3_info(
        &state.pool,
        upload_id,
        &presigned_urls.upload_id_s3,
        &complete_url,
        urls_expire_at,
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(PresignResponse { urls, complete_url }))
}

pub async fn presign_refresh(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
) -> Result<Json<PresignResponse>, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let object_key = upload.object_key.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::ValidationError,
                    message: "upload has no object key".to_string(),
                    details: None,
                },
            }),
        )
    })?;

    let part_count = upload.part_count as usize;
    let part_md5s: Vec<String> = (0..part_count).map(|_| String::new()).collect();

    let presigned_urls = generate_presigned_urls(
        &state.s3_client,
        &state.config.s3_bucket,
        &object_key,
        &part_md5s,
        state.config.presigned_ttl,
    )
    .await
    .map_err(internal_error)?;

    let urls: Vec<String> = presigned_urls.urls.clone();
    let complete_url = build_complete_url(&state.config.s3_bucket,
        &object_key,
        &presigned_urls.upload_id_s3,
    );

    let urls_expire_at =
        Utc::now() + chrono::Duration::from_std(state.config.presigned_ttl).unwrap();

    update_s3_info(
        &state.pool,
        upload_id,
        &presigned_urls.upload_id_s3,
        &complete_url,
        urls_expire_at,
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(PresignResponse { urls, complete_url }))
}

pub async fn cancel(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(upload_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let upload = get_upload(&state.pool, upload_id)
        .await
        .map_err(internal_error)?;

    let upload = match upload {
        Some(u) if u.user_id == user_id => u,
        Some(_) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Forbidden,
                        message: "not your upload".to_string(),
                        details: None,
                    },
                }),
            ))
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "upload not found".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    };

    let current_status = parse_status(&upload.status);
    validate_transition(current_status, UploadStatus::Failed).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::InvalidStateTransition,
                    message: e.message,
                    details: None,
                },
            }),
        )
    })?;

    patch_upload_status(&state.pool, upload_id, "failed")
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

pub async fn list_pending(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ListQueryParams>,
) -> Result<Json<Vec<UploadState>>, (StatusCode, Json<ErrorResponse>)> {
    let uploads =
        crate::db::uploads::list_uploads_for_user(&state.pool, user_id, query.status.as_deref())
            .await
            .map_err(internal_error)?;

    let states: Vec<UploadState> = uploads.into_iter().map(upload_to_state).collect();
    Ok(Json(states))
}

#[derive(Debug, serde::Deserialize)]
pub struct ListQueryParams {
    pub status: Option<String>,
}

struct PresignedUrls {
    upload_id_s3: String,
    urls: Vec<String>,
}

async fn generate_presigned_urls(
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
    object_key: &str,
    part_md5s: &[String],
    ttl: std::time::Duration,
) -> Result<PresignedUrls, crate::error::ZooError> {
    let create_resp = s3_client
        .create_multipart_upload()
        .bucket(bucket)
        .key(object_key)
        .send()
        .await
        .map_err(|e| crate::error::ZooError::S3(e.to_string()))?;

    let upload_id = create_resp.upload_id().unwrap_or_default().to_string();

    let mut urls = Vec::with_capacity(part_md5s.len());
    for (i, _) in part_md5s.iter().enumerate() {
        let url = presign_part_upload(
            s3_client,
            bucket,
            object_key,
            &upload_id,
            (i + 1) as i32,
            ttl,
        )
        .await?;
        urls.push(url);
    }

    Ok(PresignedUrls {
        upload_id_s3: upload_id,
        urls,
    })
}

async fn get_upload_state(
    pool: &sqlx::PgPool,
    upload_id: Uuid,
) -> Result<UploadState, crate::error::ZooError> {
    let upload = get_upload(pool, upload_id)
        .await?
        .ok_or(crate::error::ZooError::NotFound)?;
    Ok(upload_to_state(upload))
}

fn upload_to_state(u: crate::db::models::Upload) -> UploadState {
    use base64::Engine;

    let _uploaded_parts = u
        .parts_bitmask
        .as_ref()
        .map(|b| b.iter().map(|byte| byte.count_ones() as u16).sum::<u16>())
        .unwrap_or(0);

    UploadState {
        upload_id: u.id.to_string(),
        user_id: u.user_id.to_string(),
        device_id: u.device_id.to_string(),
        status: parse_status(&u.status),
        file_hash: u.file_hash,
        file_size: u.file_size,
        mime_type: u.mime_type,
        part_size: u.part_size,
        part_count: u.part_count as u16,
        parts_bitmask: u
            .parts_bitmask
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .unwrap_or_default(),
        object_key: u.object_key,
        upload_id_s3: u.upload_id_s3,
        complete_url: u.complete_url,
        urls_expire_at: u.urls_expire_at.map(|t| t.timestamp_millis()),
        last_heartbeat_at: u.last_heartbeat_at.map(|t| t.timestamp_millis()),
        stalled_at: u.stalled_at.map(|t| t.timestamp_millis()),
        error_reason: u.error_reason,
        created_at: u.created_at.timestamp_millis(),
        expires_at: u.expires_at.timestamp_millis(),
        done_at: u.done_at.map(|t| t.timestamp_millis()),
    }
}

fn parse_status(s: &str) -> UploadStatus {
    match s {
        "pending" => UploadStatus::Pending,
        "encrypting" => UploadStatus::Encrypting,
        "uploading" => UploadStatus::Uploading,
        "s3_completed" => UploadStatus::S3Completed,
        "registering" => UploadStatus::Registering,
        "done" => UploadStatus::Done,
        "stalled" => UploadStatus::Stalled,
        "resuming" => UploadStatus::Resuming,
        _ => UploadStatus::Failed,
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct PartConfirmRequest {
    pub etag: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct FailRequest {
    pub reason: Option<String>,
}

fn validation_error(msg: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: ApiError {
                code: ErrorCode::ValidationError,
                message: msg,
                details: None,
            },
        }),
    )
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
