use crate::db::collections::{create_collection, delete_collection, list_collections, update_collection};
use crate::error::ZooError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use types::error::{ApiError, ErrorCode, ErrorResponse};
use uuid::Uuid;

pub async fn create(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<Json<CollectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = create_collection(
        &state.pool,
        user_id,
        &req.encrypted_name,
        &req.encrypted_key,
        &req.key_decryption_nonce,
        req.encrypted_metadata.as_deref(),
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(CollectionResponse {
        collection_id: id.to_string(),
    }))
}

pub async fn list(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Result<Json<Vec<CollectionItem>>, (StatusCode, Json<ErrorResponse>)> {
    let since = query.since_time.unwrap_or(0);
    let collections = list_collections(&state.pool, user_id, since)
        .await
        .map_err(internal_error)?;

    let items = collections
        .into_iter()
        .map(|c| CollectionItem {
            id: c.id.to_string(),
            encrypted_name: c.encrypted_name,
            encrypted_key: c.encrypted_key,
            key_decryption_nonce: c.key_decryption_nonce,
            encrypted_metadata: c.encrypted_metadata,
            updation_time: c.updation_time.timestamp_millis(),
        })
        .collect();

    Ok(Json(items))
}

pub async fn update(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(collection_id): axum::extract::Path<Uuid>,
    Json(req): Json<UpdateCollectionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    update_collection(
        &state.pool,
        user_id,
        collection_id,
        req.encrypted_name.as_deref(),
        req.encrypted_metadata.as_deref(),
    )
    .await
    .map_err(|e| match e {
        ZooError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ApiError {
                    code: ErrorCode::NotFound,
                    message: "collection not found".to_string(),
                    details: None,
                },
            }),
        ),
        _ => internal_error(e),
    })?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
    axum::extract::Path(collection_id): axum::extract::Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    delete_collection(&state.pool, user_id, collection_id)
        .await
        .map_err(|e| match e {
            ZooError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: ApiError {
                        code: ErrorCode::NotFound,
                        message: "collection not found".to_string(),
                        details: None,
                    },
                }),
            ),
            _ => internal_error(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateCollectionRequest {
    pub encrypted_name: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub encrypted_metadata: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateCollectionRequest {
    pub encrypted_name: Option<String>,
    pub encrypted_metadata: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListQuery {
    pub since_time: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct CollectionResponse {
    pub collection_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CollectionItem {
    pub id: String,
    pub encrypted_name: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub encrypted_metadata: Option<String>,
    pub updation_time: i64,
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
