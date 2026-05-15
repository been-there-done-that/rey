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
