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
            listen_addr: std::env::var("LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3002".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/zoo".to_string()),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            s3_bucket: std::env::var("S3_BUCKET").unwrap_or_else(|_| "rey-files".to_string()),
            s3_access_key: std::env::var("S3_ACCESS_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string()),
            s3_secret_key: std::env::var("S3_SECRET_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clear_env_vars() {
        std::env::remove_var("LISTEN_ADDR");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("S3_ENDPOINT");
        std::env::remove_var("S3_REGION");
        std::env::remove_var("S3_BUCKET");
        std::env::remove_var("S3_ACCESS_KEY");
        std::env::remove_var("S3_SECRET_KEY");
        std::env::remove_var("SESSION_TTL_DAYS");
        std::env::remove_var("PRESIGNED_TTL_HOURS");
        std::env::remove_var("STALL_TIMEOUT_SECONDS");
        std::env::remove_var("GC_INTERVAL_SECONDS");
        std::env::remove_var("MAX_FILE_SIZE");
        std::env::remove_var("DEFAULT_PART_SIZE");
    }

    #[test]
    fn test_from_env_defaults() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env_vars();

        let config = ZooConfig::from_env();
        assert_eq!(config.listen_addr, "0.0.0.0:3002");
        assert_eq!(config.s3_region, "us-east-1");
        assert_eq!(config.s3_bucket, "rey-files");
        assert_eq!(config.s3_access_key, "minioadmin");
        assert_eq!(config.s3_secret_key, "minioadmin");
        assert_eq!(config.session_ttl, Duration::from_secs(30 * 86400));
        assert_eq!(config.stall_timeout, Duration::from_secs(90));
        assert_eq!(config.gc_interval, Duration::from_secs(300));
        assert_eq!(config.max_file_size, 10 * 1024 * 1024 * 1024);
        assert_eq!(config.default_part_size, 20 * 1024 * 1024);
        assert!(config.s3_endpoint.is_none());
        match &config.download_mode {
            DownloadMode::Redirect { presigned_ttl } => {
                assert_eq!(*presigned_ttl, Duration::from_secs(24 * 3600));
            }
            DownloadMode::Proxy { .. } => panic!("expected Redirect mode"),
        }
    }

    #[test]
    fn test_from_env_custom_values() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env_vars();

        std::env::set_var("LISTEN_ADDR", "127.0.0.1:8080");
        std::env::set_var("S3_REGION", "eu-west-1");
        std::env::set_var("S3_BUCKET", "my-bucket");
        std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
        std::env::set_var("SESSION_TTL_DAYS", "7");
        std::env::set_var("STALL_TIMEOUT_SECONDS", "120");
        std::env::set_var("GC_INTERVAL_SECONDS", "600");
        std::env::set_var("MAX_FILE_SIZE", "5368709120");
        std::env::set_var("DEFAULT_PART_SIZE", "10485760");

        let config = ZooConfig::from_env();
        assert_eq!(config.listen_addr, "127.0.0.1:8080");
        assert_eq!(config.s3_region, "eu-west-1");
        assert_eq!(config.s3_bucket, "my-bucket");
        assert_eq!(config.s3_endpoint, Some("http://localhost:9000".to_string()));
        assert_eq!(config.session_ttl, Duration::from_secs(7 * 86400));
        assert_eq!(config.stall_timeout, Duration::from_secs(120));
        assert_eq!(config.gc_interval, Duration::from_secs(600));
        assert_eq!(config.max_file_size, 5368709120);
        assert_eq!(config.default_part_size, 10485760);
    }

    #[test]
    fn test_from_env_invalid_parse_falls_back_to_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env_vars();

        std::env::set_var("SESSION_TTL_DAYS", "not_a_number");
        std::env::set_var("STALL_TIMEOUT_SECONDS", "abc");

        let config = ZooConfig::from_env();
        assert_eq!(config.session_ttl, Duration::from_secs(30 * 86400));
        assert_eq!(config.stall_timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_download_mode_clone() {
        let mode = DownloadMode::Redirect {
            presigned_ttl: Duration::from_secs(3600),
        };
        let cloned = mode.clone();
        match cloned {
            DownloadMode::Redirect { presigned_ttl } => {
                assert_eq!(presigned_ttl, Duration::from_secs(3600));
            }
            _ => panic!("expected Redirect"),
        }
    }

    #[test]
    fn test_zoo_config_clone() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env_vars();

        let config = ZooConfig::from_env();
        let cloned = config.clone();
        assert_eq!(cloned.listen_addr, config.listen_addr);
        assert_eq!(cloned.s3_bucket, config.s3_bucket);
    }
}
