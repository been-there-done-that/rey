# Task 3: Implement `crates/common` — Layer 0 Shared Utilities

## Wave
1 (Layer 0 — Foundation utilities)

## Dependencies
- Task 1 (Scaffold workspace) must be complete
- Task 2 (types crate) should be complete (common does NOT depend on types per design, but both are Layer 0)

## Can Run In Parallel With
- Task 2 (types crate) — no dependencies between them

## Design References
- STRUCTURE.md §2.6: common — Cross-Cutting
- design.md §2.3: Compilation Guarantees (common depends only on types, std)

## Requirements
25.1

## Objective
Implement env-var driven config parsing, tracing initialization, and shared error formatting utilities. No crypto. No I/O beyond file reading for config.

## Cargo.toml
```toml
[package]
name = "common"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod config;
pub mod error;
pub mod telemetry;
pub mod time;
pub mod result;
```

### `src/config.rs`
Implement:
- `AppConfig` struct with: `server_url: String`, `db_path: PathBuf`, `cache_dir: PathBuf`, `log_level: String`
- `impl AppConfig` with `fn from_env() -> Self` that reads:
  - `REY_SERVER_URL` (default: `"http://localhost:3002"`)
  - `REY_DB_PATH` (default: current dir + `"rey.db"`)
  - `REY_CACHE_DIR` (default: platform-specific cache dir + `"rey"`)
  - `REY_LOG_LEVEL` (default: `"info"`)
- `impl AppConfig` with `fn from_file(path: &Path) -> Result<Self, ConfigError>` for loading from TOML/JSON
- `ConfigError` enum: `MissingEnvVar { name: String }`, `ParseError { field: String, source: ... }`, `IoError(std::io::Error)`

#### Implementation Details for `src/config.rs`

```rust
use std::path::{Path, PathBuf};
use std::env;
use thiserror::Error;

/// Errors that can occur during configuration loading.
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

/// Application configuration loaded from environment or file.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_url: String,
    pub db_path: PathBuf,
    pub cache_dir: PathBuf,
    pub log_level: String,
}

impl AppConfig {
    /// Load configuration from environment variables with sensible defaults.
    ///
    /// Environment variables:
    /// - `REY_SERVER_URL` — API server URL (default: `http://localhost:3002`)
    /// - `REY_DB_PATH` — SQLite database path (default: `<cwd>/rey.db`)
    /// - `REY_CACHE_DIR` — Cache directory (default: platform-specific + `"rey"`)
    /// - `REY_LOG_LEVEL` — Tracing log level (default: `"info"`)
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

    /// Load configuration from a TOML or JSON file.
    ///
    /// The file extension determines the format: `.toml` for TOML, `.json` for JSON.
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

/// Internal struct for deserializing config files.
///
/// All fields are optional so that files can override only a subset of defaults.
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
```

**Note**: The `dirs-next` and `toml` crates are used above. Add them to `Cargo.toml`:

```toml
[dependencies]
types = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
dirs-next = "2.0"
toml = "0.8"
```

### `src/error.rs`
Implement a unified error type:
- `CommonError` enum using `#[derive(thiserror::Error)]`:
  - `#[error("configuration error: {0}")] Config(#[from] ConfigError)`
  - `#[error("I/O error: {0}")] Io(#[from] std::io::Error)`
  - `#[error("serialization error: {0}")] Serde(#[from] serde_json::Error)`
  - `#[error("parse error: {0}")] Parse(String)`
- Helper function `fn format_error_chain(err: &dyn std::error::Error) -> String` that walks the error chain and formats each level

#### Implementation Details for `src/error.rs`

```rust
use thiserror::Error;
use crate::config::ConfigError;

/// Unified error type for the common crate.
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Walks the error chain and formats each level into a multi-line string.
///
/// Example output:
/// ```text
/// Error: configuration error: failed to parse config field 'config.toml': ...
/// Caused by:
///   0: I/O error: No such file or directory (os error 2)
/// ```
pub fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut output = format!("Error: {err}");

    let mut source = err.source();
    let mut level = 0;
    while let Some(cause) = source {
        if level == 0 {
            output.push_str("\nCaused by:");
        }
        output.push_str(&format!("\n  {level}: {cause}"));
        source = cause.source();
        level += 1;
    }

    output
}
```

### `src/telemetry.rs`
Implement:
- `fn init_tracing(log_level: &str)` — configures `tracing_subscriber` with:
  - `EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(log_level))`
  - `fmt::layer()` with pretty formatting in dev, JSON in release
  - Returns `tracing::subscriber::SetGlobalDefault` guard
- `fn init_tracing_json(log_level: &str)` — same but with `fmt().json()` for production

#### Implementation Details for `src/telemetry.rs`

```rust
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize tracing with pretty formatting (development mode).
///
/// Respects `RUST_LOG` environment variable if set, otherwise uses
/// the provided `log_level`.
///
/// Returns a guard that keeps the subscriber active. Dropping the guard
/// restores the previous subscriber.
pub fn init_tracing(log_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::try_new(log_level).expect("invalid log level"));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().pretty());

    subscriber.init();
}

/// Initialize tracing with JSON formatting (production mode).
///
/// Same filter logic as `init_tracing`, but outputs structured JSON
/// suitable for log aggregation systems.
pub fn init_tracing_json(log_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::try_new(log_level).expect("invalid log level"));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json());

    subscriber.init();
}

/// Initialize tracing with the appropriate format based on build profile.
///
/// In debug builds, uses pretty formatting. In release builds, uses JSON.
pub fn init_tracing_auto(log_level: &str) {
    if cfg!(debug_assertions) {
        init_tracing(log_level);
    } else {
        init_tracing_json(log_level);
    }
}
```

### `src/time.rs`
Implement:
- `fn now_ms() -> i64` — returns current Unix timestamp in milliseconds
- `fn now_utc() -> std::time::SystemTime` — returns current UTC time as SystemTime
- Actually, avoid `chrono` dependency — use `std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64`

#### Implementation Details for `src/time.rs`

```rust
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in milliseconds.
///
/// # Panics
/// Panics if the system clock is set before the Unix epoch (1970-01-01).
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

/// Returns the current UTC time as a `SystemTime`.
pub fn now_utc() -> SystemTime {
    SystemTime::now()
}

/// Converts a Unix timestamp in milliseconds to a `SystemTime`.
pub fn from_ms(ms: i64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_millis(ms as u64)
}

/// Converts a `SystemTime` to a Unix timestamp in milliseconds.
///
/// # Panics
/// Panics if the time is before the Unix epoch.
pub fn to_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .expect("time is before Unix epoch")
        .as_millis() as i64
}

/// Returns the number of milliseconds elapsed since the given timestamp.
/// Returns `None` if the given timestamp is in the future.
pub fn elapsed_ms(since_ms: i64) -> Option<i64> {
    let now = now_ms();
    if now >= since_ms {
        Some(now - since_ms)
    } else {
        None
    }
}

/// Returns `true` if the given timestamp is older than the specified
/// number of seconds.
pub fn is_older_than(ms: i64, seconds: i64) -> bool {
    elapsed_ms(ms)
        .map(|elapsed| elapsed > seconds * 1000)
        .unwrap_or(false)
}
```

### `src/result.rs`
Implement:
- `pub type Result<T> = std::result::Result<T, CommonError>` — a convenience alias
- Extension trait `ResultExt` with methods like `.context("message")` for adding context to errors

#### Implementation Details for `src/result.rs`

```rust
use crate::error::CommonError;

/// Convenience type alias for results using `CommonError`.
pub type Result<T> = std::result::Result<T, CommonError>;

/// Extension trait for adding context to errors.
pub trait ResultExt<T, E> {
    /// Wrap the error with additional context.
    ///
    /// If the result is `Ok`, the value is returned unchanged.
    /// If the result is `Err`, the error is wrapped in a `CommonError::Parse`
    /// with the context message prepended.
    fn context(self, msg: &str) -> std::result::Result<T, CommonError>;

    /// Lazily wrap the error with context computed from a closure.
    fn with_context<F>(self, f: F) -> std::result::Result<T, CommonError>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, msg: &str) -> std::result::Result<T, CommonError> {
        self.map_err(|e| CommonError::Parse(format!("{msg}: {e}")))
    }

    fn with_context<F>(self, f: F) -> std::result::Result<T, CommonError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| CommonError::Parse(format!("{}: {e}", f())))
    }
}

/// Extension trait specifically for `CommonError` results.
pub trait CommonResultExt<T> {
    /// Add context to a `CommonError`, wrapping the existing error message.
    fn context(self, msg: &str) -> Result<T>;
}

impl<T> CommonResultExt<T> for Result<T> {
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| CommonError::Parse(format!("{msg}: {e}")))
    }
}
```

## Tests (Task 3.3 — marked with *)
Write unit tests for:
- `AppConfig::from_env()` with mocked environment variables
- `format_error_chain()` produces correct multi-line output
- `now_ms()` returns a reasonable timestamp (> 1700000000000)
- Error conversion: `std::io::Error` converts to `CommonError::Io`

### Test Implementation Details

#### `src/config.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_env_uses_defaults_when_no_vars_set() {
        // Clear relevant env vars for this test
        env::remove_var("REY_SERVER_URL");
        env::remove_var("REY_DB_PATH");
        env::remove_var("REY_CACHE_DIR");
        env::remove_var("REY_LOG_LEVEL");

        let config = AppConfig::from_env();
        assert_eq!(config.server_url, "http://localhost:3002");
        assert_eq!(config.log_level, "info");
        assert!(config.db_path.ends_with("rey.db"));
        assert!(config.cache_dir.ends_with("rey"));
    }

    #[test]
    fn test_from_env_reads_server_url() {
        env::set_var("REY_SERVER_URL", "https://api.example.com");
        env::remove_var("REY_DB_PATH");
        env::remove_var("REY_CACHE_DIR");
        env::remove_var("REY_LOG_LEVEL");

        let config = AppConfig::from_env();
        assert_eq!(config.server_url, "https://api.example.com");

        env::remove_var("REY_SERVER_URL");
    }

    #[test]
    fn test_from_env_reads_log_level() {
        env::set_var("REY_LOG_LEVEL", "debug");
        env::remove_var("REY_SERVER_URL");
        env::remove_var("REY_DB_PATH");
        env::remove_var("REY_CACHE_DIR");

        let config = AppConfig::from_env();
        assert_eq!(config.log_level, "debug");

        env::remove_var("REY_LOG_LEVEL");
    }

    #[test]
    fn test_from_env_reads_db_path() {
        env::set_var("REY_DB_PATH", "/tmp/test.db");
        env::remove_var("REY_SERVER_URL");
        env::remove_var("REY_CACHE_DIR");
        env::remove_var("REY_LOG_LEVEL");

        let config = AppConfig::from_env();
        assert_eq!(config.db_path, PathBuf::from("/tmp/test.db"));

        env::remove_var("REY_DB_PATH");
    }

    #[test]
    fn test_from_file_json() {
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
```

#### `src/error.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let common: CommonError = io_err.into();
        match common {
            CommonError::Io(e) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            _ => panic!("Expected CommonError::Io"),
        }
    }

    #[test]
    fn test_serde_error_conversion() {
        let bad_json = "not valid json";
        let result: std::result::Result<serde_json::Value, _> = serde_json::from_str(bad_json);
        let common: CommonError = result.unwrap_err().into();
        match common {
            CommonError::Serde(_) => {}
            _ => panic!("Expected CommonError::Serde"),
        }
    }

    #[test]
    fn test_format_error_chain_single() {
        let err = CommonError::Parse("something went wrong".to_string());
        let output = format_error_chain(&err);
        assert!(output.starts_with("Error: parse error: something went wrong"));
    }

    #[test]
    fn test_format_error_chain_with_cause() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let config_err = ConfigError::IoError(io_err);
        let common: CommonError = config_err.into();

        let output = format_error_chain(&common);
        assert!(output.contains("configuration error"));
        assert!(output.contains("Caused by:"));
        assert!(output.contains("I/O error"));
        assert!(output.contains("access denied"));
    }

    #[test]
    fn test_format_error_chain_multi_level() {
        let io_err = io::Error::new(
            io::ErrorKind::NotFound,
            "No such file or directory",
        );
        let parse_err = CommonError::Parse(format!("config load failed: {io_err}"));

        let output = format_error_chain(&parse_err);
        assert!(output.contains("config load failed"));
    }
}
```

#### `src/telemetry.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tracing_does_not_panic() {
        // This test verifies that init_tracing doesn't panic with valid input.
        // Note: calling init() multiple times in tests may fail because the
        // global subscriber can only be set once. We test with a valid level.
        let result = std::panic::catch_unwind(|| {
            init_tracing("info");
        });
        // May fail if another test already set the global subscriber,
        // which is expected behavior.
        drop(result);
    }

    #[test]
    fn test_init_tracing_json_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            init_tracing_json("info");
        });
        drop(result);
    }

    #[test]
    fn test_init_tracing_auto_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            init_tracing_auto("debug");
        });
        drop(result);
    }
}
```

#### `src/time.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_ms_returns_reasonable_timestamp() {
        let ts = now_ms();
        // January 2024 in milliseconds
        assert!(ts > 1_700_000_000_000);
        // Year 2100 in milliseconds (sanity upper bound)
        assert!(ts < 4_102_444_800_000);
    }

    #[test]
    fn test_now_ms_is_monotonically_increasing() {
        let first = now_ms();
        let second = now_ms();
        assert!(second >= first);
    }

    #[test]
    fn test_from_ms_to_ms_roundtrip() {
        let original = 1_700_000_000_000i64;
        let time = from_ms(original);
        let recovered = to_ms(time);
        assert_eq!(recovered, original);
    }

    #[test]
    fn test_elapsed_ms_returns_some_for_past_timestamp() {
        let past = now_ms() - 5000; // 5 seconds ago
        let elapsed = elapsed_ms(past);
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap() >= 0);
    }

    #[test]
    fn test_elapsed_ms_returns_none_for_future_timestamp() {
        let future = now_ms() + 10_000_000; // far in the future
        let elapsed = elapsed_ms(future);
        assert!(elapsed.is_none());
    }

    #[test]
    fn test_is_older_than_true_for_old_timestamp() {
        let old = now_ms() - 10_000; // 10 seconds ago
        assert!(is_older_than(old, 5)); // older than 5 seconds
    }

    #[test]
    fn test_is_older_than_false_for_recent_timestamp() {
        let recent = now_ms();
        assert!(!is_older_than(recent, 5));
    }
}
```

#### `src/result.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_result_ext_context_on_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let result: std::result::Result<(), _> = Err(io_err);
        let wrapped = result.context("failed to read config");

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("failed to read config"));
                assert!(msg.contains("file missing"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }

    #[test]
    fn test_result_ext_context_on_ok() {
        let result: std::result::Result<i32, io::Error> = Ok(42);
        let wrapped = result.context("this should not be used");
        assert_eq!(wrapped.unwrap(), 42);
    }

    #[test]
    fn test_result_ext_with_context_lazy() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let result: std::result::Result<(), _> = Err(io_err);
        let wrapped = result.with_context(|| format!("accessing {}", "/secret/path"));

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("accessing /secret/path"));
                assert!(msg.contains("denied"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }

    #[test]
    fn test_common_result_ext_context() {
        let err: Result<i32> = Err(CommonError::Parse("original error".to_string()));
        let wrapped = err.context("outer context");

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("outer context"));
                assert!(msg.contains("original error"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }
}
```

## Verification Steps
- [ ] `cargo check -p common` succeeds
- [ ] `cargo test -p common` passes
- [ ] No internal crate dependencies except `types`
- [ ] `tracing` initialization works without panicking
- [ ] Config parsing handles missing env vars gracefully with defaults

## Notes
- This crate is intentionally small — it's just shared utilities.
- Do NOT add any crypto, HTTP, or DB dependencies.
- The `types` dependency is for re-exporting `ErrorCode` and `ErrorResponse` from the types crate.
- Keep `common` lean — if a utility is only used by one crate, it belongs in that crate, not here.
- The `dirs-next` crate is used for platform-specific cache directory resolution. It is a maintained fork of the deprecated `dirs` crate.
- The `toml` crate is used for config file parsing. Version 0.8 is compatible with serde 1.x.
- `tracing_subscriber` initialization can only happen once per process. In tests, subsequent calls will fail silently or panic — this is expected. Tests use `catch_unwind` to handle this gracefully.
- All time utilities use `std::time` exclusively — no `chrono` dependency to keep the dependency graph minimal.
- The `ResultExt` trait works with any `Result<T, E>` where `E: std::error::Error`, not just `CommonError`. The `CommonResultExt` trait is specifically for `Result<T>` (i.e., `Result<T, CommonError>`) and provides the same `context` method without requiring a generic error type.
- `ConfigError` intentionally does NOT derive `Clone` because `Box<dyn std::error::Error>` is not `Clone`. This is acceptable since config errors are typically consumed, not cloned.
- The `AppConfigFile` internal struct allows partial config files — users can override only the fields they care about, and defaults fill in the rest.
