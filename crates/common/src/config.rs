use std::path::{Path, PathBuf};
use std::env;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("missing environment variable: {name}")]
    MissingEnvVar { name: String },

    #[error("failed to parse config field '{field}': {source}")]
    ParseError {
        field: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_url: String,
    pub db_path: PathBuf,
    pub cache_dir: PathBuf,
    pub log_level: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let server_url = env::var("REY_SERVER_URL")
            .unwrap_or_else(|_| "http://localhost:3002".to_string());

        let db_path = env::var("REY_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                path.push("rey.db");
                path
            });

        let cache_dir = env::var("REY_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let base = dirs_next::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
                base.join("rey")
            });

        let log_level = env::var("REY_LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string());

        Self {
            server_url,
            db_path,
            cache_dir,
            log_level,
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;

        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => {
                let config: AppConfigFile = toml::from_str(&content).map_err(|e| ConfigError::ParseError {
                    field: path.display().to_string(),
                    source: Box::new(e),
                })?;
                Ok(config.into_app_config())
            }
            Some("json") => {
                let config: AppConfigFile = serde_json::from_str(&content).map_err(|e| ConfigError::ParseError {
                    field: path.display().to_string(),
                    source: Box::new(e),
                })?;
                Ok(config.into_app_config())
            }
            _ => Err(ConfigError::ParseError {
                field: "extension".to_string(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unsupported config file format. Use .toml or .json",
                )),
            }),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct AppConfigFile {
    server_url: Option<String>,
    db_path: Option<String>,
    cache_dir: Option<String>,
    log_level: Option<String>,
}

impl AppConfigFile {
    fn into_app_config(self) -> AppConfig {
        let defaults = AppConfig::from_env();

        AppConfig {
            server_url: self.server_url.unwrap_or(defaults.server_url),
            db_path: self.db_path.map(PathBuf::from).unwrap_or(defaults.db_path),
            cache_dir: self.cache_dir.map(PathBuf::from).unwrap_or(defaults.cache_dir),
            log_level: self.log_level.unwrap_or(defaults.log_level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clear_env() {
        env::remove_var("REY_SERVER_URL");
        env::remove_var("REY_DB_PATH");
        env::remove_var("REY_CACHE_DIR");
        env::remove_var("REY_LOG_LEVEL");
    }

    #[test]
    fn test_from_env_uses_defaults_when_no_vars_set() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();

        let config = AppConfig::from_env();
        assert_eq!(config.server_url, "http://localhost:3002");
        assert_eq!(config.log_level, "info");
        assert!(config.db_path.ends_with("rey.db"));
        assert!(config.cache_dir.ends_with("rey"));
    }

    #[test]
    fn test_from_env_reads_server_url() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();
        env::set_var("REY_SERVER_URL", "https://api.example.com");

        let config = AppConfig::from_env();
        assert_eq!(config.server_url, "https://api.example.com");
    }

    #[test]
    fn test_from_env_reads_log_level() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();
        env::set_var("REY_LOG_LEVEL", "debug");

        let config = AppConfig::from_env();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn test_from_env_reads_db_path() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();
        env::set_var("REY_DB_PATH", "/tmp/test.db");

        let config = AppConfig::from_env();
        assert_eq!(config.db_path, PathBuf::from("/tmp/test.db"));
    }

    #[test]
    fn test_from_file_json() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_config.json");
        let content = r#"{
            "server_url": "https://test.example.com",
            "log_level": "warn"
        }"#;
        std::fs::write(&path, content).unwrap();

        let config = AppConfig::from_file(&path).unwrap();
        assert_eq!(config.server_url, "https://test.example.com");
        assert_eq!(config.log_level, "warn");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_from_file_toml() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_env();
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_config.toml");
        let content = r#"
server_url = "https://test.example.com"
log_level = "trace"
"#;
        std::fs::write(&path, content).unwrap();

        let config = AppConfig::from_file(&path).unwrap();
        assert_eq!(config.server_url, "https://test.example.com");
        assert_eq!(config.log_level, "trace");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_from_file_unsupported_extension() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_config.yaml");
        std::fs::write(&path, "key: value").unwrap();

        let result = AppConfig::from_file(&path);
        assert!(result.is_err());

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_from_file_not_found() {
        let result = AppConfig::from_file(Path::new("/nonexistent/path/config.json"));
        assert!(result.is_err());
    }
}
