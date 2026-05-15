use thiserror::Error;

#[derive(Error, Debug)]
pub enum LocalDbError {
    #[error("keychain unavailable")]
    KeychainUnavailable,
    #[error("invalid database key")]
    InvalidKey,
    #[error("migration failed: {0}")]
    MigrationFailed(String),
    #[error("query error: {0}")]
    QueryError(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
