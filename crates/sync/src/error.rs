#[derive(thiserror::Error, Debug)]
pub enum SyncError {
    #[error("network error: {0}")]
    NetworkError(#[from] zoo_client::ZooError),

    #[error("decryption failed for file {file_id}: {source}")]
    DecryptionFailed {
        file_id: i64,
        source: crypto::error::CryptoError,
    },

    #[error("database error: {0}")]
    DbError(#[from] local_db::error::LocalDbError),

    #[error("cursor error: {0}")]
    CursorError(String),

    #[error("offline mode: no network available")]
    Offline,

    #[error("parse error: {0}")]
    ParseError(String),
}
