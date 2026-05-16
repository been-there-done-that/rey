use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use zoo::api::create_router;
use zoo::config::ZooConfig;
use zoo::s3::client::create_client;
use zoo::sse::hub::SseHub;
use zoo::workers::garbage_collector::GarbageCollector;
use zoo::workers::stall_detector::StallDetector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = ZooConfig::from_env();
    tracing::info!("starting zoo server on {}", config.listen_addr);

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    tracing::info!("running migrations");
    sqlx::migrate!("./migrations").run(&pool).await?;

    let s3_client = create_client(&config).await?;
    tracing::info!("connected to s3");

    let sse_hub = Arc::new(SseHub::new());

    let gc = GarbageCollector::new(
        pool.clone(),
        s3_client.clone(),
        sse_hub.clone(),
        config.s3_bucket.clone(),
        config.gc_interval,
    );

    let stall_detector = StallDetector::new(pool.clone(), sse_hub.clone(), config.stall_timeout);

    tokio::spawn(async move {
        gc.run().await;
    });

    tokio::spawn(async move {
        stall_detector.run().await;
    });

    let app = create_router(pool, s3_client, config.clone(), sse_hub);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!("listening on {}", config.listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
