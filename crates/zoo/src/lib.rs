pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod rate_limit;
pub mod s3;
pub mod sse;
pub mod state;
pub mod types;
pub mod validation;
pub mod workers;

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;

pub async fn create_app(database_url: &str, config: config::ZooConfig) -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let s3_client = s3::client::create_client(&config).await?;

    let sse_hub = Arc::new(sse::hub::SseHub::new());

    let gc = workers::garbage_collector::GarbageCollector::new(
        pool.clone(),
        s3_client.clone(),
        sse_hub.clone(),
        config.s3_bucket.clone(),
        config.gc_interval,
    );

    let stall_detector =
        workers::stall_detector::StallDetector::new(pool.clone(), sse_hub.clone(), config.stall_timeout);

    tokio::spawn(async move {
        gc.run().await;
    });

    tokio::spawn(async move {
        stall_detector.run().await;
    });

    let app = api::create_router(pool, s3_client, config, sse_hub);

    Ok(app)
}

pub async fn listen_on(app: Router, addr: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn config_from_env() -> config::ZooConfig {
    config::ZooConfig::from_env()
}

pub const DEFAULT_PART_SIZE: i64 = 20 * 1024 * 1024;
pub const MAX_FILE_SIZE: i64 = 10 * 1024 * 1024 * 1024;
