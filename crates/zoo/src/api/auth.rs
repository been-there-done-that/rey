use crate::db::sessions::{create_session, revoke_session};
use crate::db::users::{find_user_by_email, register_user};
use crate::error::ZooError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use bcrypt::{hash, verify, DEFAULT_COST};
use sha2::{Digest, Sha256};
use types::error::{ApiError, ErrorCode, ErrorResponse};
use types::user::{LoginParams, LoginRequest, LoginResponse, UserRegistration};
use uuid::Uuid;

pub async fn get_login_params(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginParams>, (StatusCode, Json<ErrorResponse>)> {
    let user = find_user_by_email(&state.pool, &req.email)
        .await
        .map_err(internal_error)?;

    match user {
        Some(u) => Ok(Json(LoginParams {
            kek_salt: u.kek_salt,
            mem_limit: u.mem_limit as u32,
            ops_limit: u.ops_limit as u32,
        })),
        None => {
            dummy_login_delay();
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Unauthorized,
                        message: "invalid credentials".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user = find_user_by_email(&state.pool, &req.email)
        .await
        .map_err(internal_error)?;

    match user {
        Some(u) => {
            let valid = verify(&req.verify_key_hash, &u.verify_key_hash)
                .map_err(|e| internal_error(ZooError::Internal(e.to_string())))?;

            if !valid {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: ApiError {
                            code: ErrorCode::Unauthorized,
                            message: "invalid credentials".to_string(),
                            details: None,
                        },
                    }),
                ));
            }

            let session_token = Uuid::new_v4().to_string();
            let token_hash = format!("{:x}", Sha256::digest(session_token.as_bytes()));
            let expires_at =
                chrono::Utc::now() + chrono::Duration::from_std(state.config.session_ttl).unwrap();

            create_session(&state.pool, u.id, &token_hash, expires_at)
                .await
                .map_err(internal_error)?;

            Ok(Json(LoginResponse {
                session_token,
                key_attributes: types::crypto::KeyAttributes {
                    encrypted_master_key: u.encrypted_master_key,
                    key_nonce: u.key_nonce,
                    kek_salt: u.kek_salt,
                    mem_limit: u.mem_limit as u32,
                    ops_limit: u.ops_limit as u32,
                },
            }))
        }
        None => {
            dummy_bcrypt();
            dummy_login_delay();
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::Unauthorized,
                        message: "invalid credentials".to_string(),
                        details: None,
                    },
                }),
            ))
        }
    }
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<UserRegistration>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    register_user(
        &state.pool,
        &req.email,
        &req.verify_key_hash,
        &req.encrypted_master_key,
        &req.key_nonce,
        &req.kek_salt,
        req.mem_limit as i32,
        req.ops_limit as i32,
        &req.public_key,
        &req.encrypted_secret_key,
        &req.secret_key_nonce,
        &req.encrypted_recovery_key,
        &req.recovery_key_nonce,
    )
    .await
    .map_err(|e| match e {
        crate::error::ZooError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::ValidationError,
                    message: "email already exists".to_string(),
                    details: None,
                },
            }),
        ),
        _ => internal_error(e),
    })?;

    Ok(StatusCode::CREATED)
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let auth_header = headers.get(axum::http::header::AUTHORIZATION).ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: ApiError {
                code: ErrorCode::Unauthorized,
                message: "missing authorization header".to_string(),
                details: None,
            },
        }),
    ))?;

    let auth_str = auth_header.to_str().map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::Unauthorized,
                    message: "invalid token".to_string(),
                    details: None,
                },
            }),
        )
    })?;

    let token = &auth_str[7..];
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));

    revoke_session(&state.pool, &token_hash)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

fn dummy_bcrypt() {
    let _ = hash("dummy", DEFAULT_COST);
}

fn dummy_login_delay() {
    std::thread::sleep(std::time::Duration::from_millis(100));
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
