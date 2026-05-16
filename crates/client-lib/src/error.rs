#[derive(thiserror::Error, Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "message")]
pub enum CommandError {
    #[error("not logged in")]
    NotLoggedIn,

    #[error("sync error: {0}")]
    SyncError(String),

    #[error("database error: {0}")]
    DbError(String),

    #[error("zoo error: {0}")]
    ZooError(String),

    #[error("crypto error: {0}")]
    CryptoError(String),

    #[error("thumbnail error: {0}")]
    ThumbnailError(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("app error: {0}")]
    AppError(String),
}

impl From<sync::SyncError> for CommandError {
    fn from(e: sync::SyncError) -> Self {
        CommandError::SyncError(e.to_string())
    }
}

impl From<local_db::LocalDbError> for CommandError {
    fn from(e: local_db::LocalDbError) -> Self {
        CommandError::DbError(e.to_string())
    }
}

impl From<zoo_client::ZooError> for CommandError {
    fn from(e: zoo_client::ZooError) -> Self {
        CommandError::ZooError(e.to_string())
    }
}

impl From<crypto::error::CryptoError> for CommandError {
    fn from(e: crypto::error::CryptoError) -> Self {
        CommandError::CryptoError(e.to_string())
    }
}

impl From<thumbnail::ThumbnailError> for CommandError {
    fn from(e: thumbnail::ThumbnailError) -> Self {
        CommandError::ThumbnailError(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError::Io(e.to_string())
    }
}

impl From<crate::state::AppError> for CommandError {
    fn from(e: crate::state::AppError) -> Self {
        CommandError::AppError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_error_display_not_logged_in() {
        let err = CommandError::NotLoggedIn;
        assert_eq!(format!("{}", err), "not logged in");
    }

    #[test]
    fn test_command_error_display_sync() {
        let err = CommandError::SyncError("sync failed".to_string());
        assert_eq!(format!("{}", err), "sync error: sync failed");
    }

    #[test]
    fn test_command_error_display_validation() {
        let err = CommandError::Validation("invalid input".to_string());
        assert_eq!(format!("{}", err), "validation error: invalid input");
    }

    #[test]
    fn test_command_error_serializes_with_tag() {
        let err = CommandError::NotLoggedIn;
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"type\":\"NotLoggedIn\""));
    }

    #[test]
    fn test_command_error_serializes_with_message() {
        let err = CommandError::DbError("connection failed".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"type\":\"DbError\""));
        assert!(json.contains("connection failed"));
    }

    #[test]
    fn test_command_error_debug_output() {
        let err = CommandError::ZooError("timeout".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ZooError"));
        assert!(debug.contains("timeout"));
    }

    #[test]
    fn test_from_crypto_error() {
        let crypto_err = crypto::error::CryptoError::MacMismatch;
        let cmd_err: CommandError = crypto_err.into();
        assert!(matches!(cmd_err, CommandError::CryptoError(_)));
    }

    #[test]
    fn test_from_thumbnail_error() {
        let thumb_err = thumbnail::ThumbnailError::NotFound;
        let cmd_err: CommandError = thumb_err.into();
        assert!(matches!(cmd_err, CommandError::ThumbnailError(_)));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let cmd_err: CommandError = io_err.into();
        assert!(matches!(cmd_err, CommandError::Io(_)));
    }

    #[test]
    fn test_from_zoo_error() {
        let zoo_err = zoo_client::ZooError::NotAuthenticated;
        let cmd_err: CommandError = zoo_err.into();
        assert!(matches!(cmd_err, CommandError::ZooError(_)));
    }
}
