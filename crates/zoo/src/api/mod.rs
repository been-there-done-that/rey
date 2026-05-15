use crate::auth::middleware::auth_middleware;
use crate::state::AppState;
use axum::middleware;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use std::sync::Arc;

pub mod auth;
pub mod devices;
pub mod events;
pub mod files;
pub mod sync;
pub mod uploads;

pub fn create_router(
    pool: sqlx::PgPool,
    s3_client: aws_sdk_s3::Client,
    config: crate::config::ZooConfig,
    sse_hub: Arc<crate::sse::hub::SseHub>,
) -> Router {
    let app_state = AppState {
        pool: pool.clone(),
        s3_client: s3_client.clone(),
        config: config.clone(),
        sse_hub: sse_hub.clone(),
    };

    let public_routes = Router::new()
        .route("/api/auth/login-params", post(auth::get_login_params))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/register", post(auth::register));

    let protected_routes = Router::new()
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/devices", post(devices::register))
        .route("/api/devices/{device_id}", delete(devices::deregister))
        .route(
            "/api/devices/{device_id}/heartbeat",
            post(devices::heartbeat),
        )
        .route("/api/events", get(events::sse_stream))
        .route("/api/events/test", post(events::send_test_event))
        .route("/api/files/{file_id}/download", get(files::get_download_url))
        .route("/api/files/{file_id}/archive", put(files::archive))
        .route("/api/sync/files", get(sync::sync_files))
        .route("/api/uploads", post(uploads::create))
        .route("/api/uploads", get(uploads::list_pending))
        .route("/api/uploads/{upload_id}", get(uploads::get_status))
        .route("/api/uploads/{upload_id}", patch(uploads::patch_status))
        .route("/api/uploads/{upload_id}", delete(uploads::cancel))
        .route(
            "/api/uploads/{upload_id}/heartbeat",
            post(uploads::heartbeat),
        )
        .route("/api/uploads/{upload_id}/complete", post(uploads::complete))
        .route("/api/uploads/{upload_id}/presign", post(uploads::presign))
        .route(
            "/api/uploads/{upload_id}/presign-refresh",
            post(uploads::presign_refresh),
        )
        .route(
            "/api/uploads/{upload_id}/register",
            post(uploads::register_file),
        )
        .route("/api/uploads/{upload_id}/fail", post(uploads::fail))
        .route(
            "/api/uploads/{upload_id}/parts/{part_number}",
            put(uploads::confirm_part),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(app_state)
}
