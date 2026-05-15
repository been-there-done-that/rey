use std::time::Duration;

#[derive(Debug, Clone)]
pub enum DownloadMode {
    Redirect { presigned_ttl: Duration },
    Proxy { max_concurrent: usize },
}

#[derive(Debug, Clone)]
pub struct ZooConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub s3_endpoint: Option<String>,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub session_ttl: Duration,
    pub download_mode: DownloadMode,
    pub stall_timeout: Duration,
    pub presigned_ttl: Duration,
    pub gc_interval: Duration,
    pub max_file_size: u64,
    pub default_part_size: u32,
}

impl ZooConfig {
    pub fn from_env() -> Self {
        Self {
            listen_addr: std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3002".to_string()),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/zoo".to_string()),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            s3_bucket: std::env::var("S3_BUCKET").unwrap_or_else(|_| "rey-files".to_string()),
            s3_access_key: std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            s3_secret_key: std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            session_ttl: Duration::from_secs(
                std::env::var("SESSION_TTL_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(30)
                    * 86400,
            ),
            download_mode: DownloadMode::Redirect {
                presigned_ttl: Duration::from_secs(
                    std::env::var("PRESIGNED_TTL_HOURS")
                        .ok()
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(24)
                        * 3600,
                ),
            },
            stall_timeout: Duration::from_secs(
                std::env::var("STALL_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(90),
            ),
            presigned_ttl: Duration::from_secs(
                std::env::var("PRESIGNED_TTL_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(24)
                    * 3600,
            ),
            gc_interval: Duration::from_secs(
                std::env::var("GC_INTERVAL_SECONDS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(300),
            ),
            max_file_size: std::env::var("MAX_FILE_SIZE")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(10 * 1024 * 1024 * 1024),
            default_part_size: std::env::var("DEFAULT_PART_SIZE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(20 * 1024 * 1024),
        }
    }
}
