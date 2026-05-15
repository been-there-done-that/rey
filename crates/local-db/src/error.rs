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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;

    #[test]
    fn test_display_keychain_unavailable() {
        let err = LocalDbError::KeychainUnavailable;
        assert_eq!(format!("{}", err), "keychain unavailable");
    }

    #[test]
    fn test_display_invalid_key() {
        let err = LocalDbError::InvalidKey;
        assert_eq!(format!("{}", err), "invalid database key");
    }

    #[test]
    fn test_display_migration_failed() {
        let err = LocalDbError::MigrationFailed("step 1 failed".to_string());
        assert_eq!(format!("{}", err), "migration failed: step 1 failed");
    }

    #[test]
    fn test_display_query_error() {
        let sqlite_err = rusqlite::Error::InvalidQuery;
        let err = LocalDbError::QueryError(sqlite_err);
        assert!(format!("{}", err).starts_with("query error:"));
    }

    #[test]
    fn test_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = LocalDbError::Io(io_err);
        assert!(format!("{}", err).contains("file not found"));
    }

    #[test]
    fn test_error_source_for_query_error() {
        let sqlite_err = rusqlite::Error::InvalidQuery;
        let err = LocalDbError::QueryError(sqlite_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_for_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let err = LocalDbError::Io(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_for_simple_errors_is_none() {
        assert!(LocalDbError::KeychainUnavailable.source().is_none());
        assert!(LocalDbError::InvalidKey.source().is_none());
        let migration = LocalDbError::MigrationFailed("test".to_string());
        assert!(migration.source().is_none());
    }

    #[test]
    fn test_debug_output() {
        let err = LocalDbError::KeychainUnavailable;
        let debug = format!("{:?}", err);
        assert!(debug.contains("KeychainUnavailable"));
    }
}
