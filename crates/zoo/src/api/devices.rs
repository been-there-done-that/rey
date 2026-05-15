use crate::db::devices::{lookup_device_by_id, lookup_by_sse_token, register_device, tombstone_device, update_last_seen};
use crate::error::ZooError;
use crate::state::AppState;
use crate::validation::validate_device_name;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use types::device::{DeviceInfo, DeviceRegistration};
use types::error::{ApiError, ErrorCode, ErrorResponse};
use uuid::Uuid;

pub async fn register(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    Json(req): Json<DeviceRegistration>,
) -> Result<(StatusCode, Json<DeviceInfo>), (StatusCode, Json<ErrorResponse>)> {
    validate_device_name(&req.name).map_err(validation_error)?;

    let sse_token = Uuid::new_v4().to_string();

    let _device_id = register_device(
        &state.pool,
        user_id,
        &req.name,
        &format!("{:?}", req.platform),
        &sse_token,
        req.push_token.as_deref(),
        90,
    )
    .await
    .map_err(|e| match e {
        ZooError::DeviceNameTaken => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::DeviceNameTaken,
                    message: "device name already taken".to_string(),
                    details: None,
                },
            }),
        ),
        _ => internal_error(e),
    })?;

    let device = lookup_by_sse_token(&state.pool, &sse_token)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            internal_error(ZooError::Internal(
                "device not found after creation".to_string(),
            ))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(DeviceInfo {
            device_id: device.id.to_string(),
            name: device.name,
            platform: types::device::DevicePlatform::Desktop,
            sse_token: device.sse_token,
            push_token: device.push_token,
            stall_timeout_seconds: device.stall_timeout_seconds as u32,
        }),
    ))
}

pub async fn deregister(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(device_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let device = lookup_device_by_id(&state.pool, device_id)
        .await
        .map_err(internal_error)?;

    match device {
        Some(d) if d.user_id == user_id => {
            tombstone_device(&state.pool, device_id)
                .await
                .map_err(internal_error)?;
            Ok(StatusCode::NO_CONTENT)
        }
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::Forbidden,
                    message: "not your device".to_string(),
                    details: None,
                },
            }),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::NotFound,
                    message: "device not found".to_string(),
                    details: None,
                },
            }),
        )),
    }
}

pub async fn heartbeat(
    State(state): State<AppState>,
    axum::extract::Path(device_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    update_last_seen(&state.pool, device_id)
        .await
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn validation_error(e: ZooError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: ApiError {
                code: ErrorCode::ValidationError,
                message: e.to_string(),
                details: None,
            },
        }),
    )
}

fn internal_error(e: ZooError) -> (StatusCode, Json<ErrorResponse>) {
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
