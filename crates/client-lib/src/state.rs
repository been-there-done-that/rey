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
        let db = Arc::new(tokio::sync::Mutex::new(LocalDb::open(Path::new(&config.db_path))?));

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
