use common::config::AppConfig;
use crypto::Key256;
use local_db::LocalDb;
use std::path::Path;
use std::sync::Arc;
use thumbnail::cache::ThumbnailCache;
use tokio::sync::RwLock;
use types::device::DeviceInfo;
use zoo_client::ZooClient;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    DbError(#[from] local_db::LocalDbError),

    #[error("thumbnail error: {0}")]
    ThumbnailError(#[from] thumbnail::ThumbnailError),

    #[error("config error: {0}")]
    ConfigError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct AppState {
    pub db: Arc<tokio::sync::Mutex<LocalDb>>,
    pub master_key: Arc<RwLock<Option<Key256>>>,
    pub session_token: Arc<RwLock<Option<String>>>,
    pub device_info: Arc<RwLock<Option<DeviceInfo>>>,
    pub thumbnail_cache: Arc<ThumbnailCache>,
    pub zoo_client: Arc<ZooClient>,
    pub config: Arc<AppConfig>,
}

impl AppState {
    pub async fn init(config: AppConfig) -> Result<Self, AppError> {
        let db = Arc::new(tokio::sync::Mutex::new(LocalDb::open(Path::new(
            &config.db_path,
        ))?));

        let cache_dir = config.cache_dir.join("thumbnails");
        std::fs::create_dir_all(&cache_dir).ok();

        let thumbnail_cache = Arc::new(
            ThumbnailCache::new(500, cache_dir, 2 * 1024 * 1024 * 1024)
                .map_err(|e| thumbnail::ThumbnailError::CacheError(e.to_string()))?,
        );

        let zoo_client = Arc::new(ZooClient::new(config.server_url.clone()));

        Ok(Self {
            db,
            master_key: Arc::new(RwLock::new(None)),
            session_token: Arc::new(RwLock::new(None)),
            device_info: Arc::new(RwLock::new(None)),
            thumbnail_cache,
            zoo_client,
            config: Arc::new(config),
        })
    }

    pub async fn set_master_key(&self, key: Key256) {
        let mut lock = self.master_key.write().await;
        *lock = Some(key);
    }

    pub async fn clear_master_key(&self) {
        let mut lock = self.master_key.write().await;
        *lock = None;
    }

    pub async fn set_session_token(&self, token: String) {
        let mut lock = self.session_token.write().await;
        *lock = Some(token.clone());
        self.zoo_client.set_session_token(token);
    }

    pub async fn clear_session_token(&self) {
        let mut lock = self.session_token.write().await;
        *lock = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;

    #[test]
    fn test_app_error_display_db() {
        let err = AppError::ConfigError("missing path".to_string());
        assert_eq!(format!("{}", err), "config error: missing path");
    }

    #[test]
    fn test_app_error_display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = AppError::IoError(io_err);
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_app_error_debug() {
        let err = AppError::ConfigError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ConfigError"));
    }

    #[tokio::test]
    async fn test_master_key_set_and_clear() {
        let key = generate_key();
        let master_key: Arc<RwLock<Option<Key256>>> = Arc::new(RwLock::new(None));

        {
            let mut lock = master_key.write().await;
            *lock = Some(key);
        }

        {
            let lock = master_key.read().await;
            assert!(lock.is_some());
        }

        {
            let mut lock = master_key.write().await;
            *lock = None;
        }

        {
            let lock = master_key.read().await;
            assert!(lock.is_none());
        }
    }

    #[tokio::test]
    async fn test_session_token_set_and_clear() {
        let token: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        {
            let mut lock = token.write().await;
            *lock = Some("test-token-123".to_string());
        }

        {
            let lock = token.read().await;
            assert_eq!(lock.as_ref().unwrap(), "test-token-123");
        }

        {
            let mut lock = token.write().await;
            *lock = None;
        }

        {
            let lock = token.read().await;
            assert!(lock.is_none());
        }
    }

    #[tokio::test]
    async fn test_device_info_set_and_clear() {
        let device_info: Arc<RwLock<Option<DeviceInfo>>> = Arc::new(RwLock::new(None));

        let device = DeviceInfo {
            device_id: "device-1".to_string(),
            name: "Test Device".to_string(),
            platform: types::device::DevicePlatform::Desktop,
            sse_token: "sse-token".to_string(),
            push_token: None,
            stall_timeout_seconds: 30,
        };

        {
            let mut lock = device_info.write().await;
            *lock = Some(device.clone());
        }

        {
            let lock = device_info.read().await;
            assert!(lock.is_some());
            assert_eq!(lock.as_ref().unwrap().name, "Test Device");
        }

        {
            let mut lock = device_info.write().await;
            *lock = None;
        }

        {
            let lock = device_info.read().await;
            assert!(lock.is_none());
        }
    }

    #[test]
    fn test_zoo_client_base_url() {
        let client = ZooClient::new("https://api.example.com".to_string());
        assert_eq!(client.base_url(), "https://api.example.com");
    }
}
