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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_error_display_offline() {
        let err = SyncError::Offline;
        assert_eq!(format!("{}", err), "offline mode: no network available");
    }

    #[test]
    fn test_sync_error_display_cursor() {
        let err = SyncError::CursorError("test cursor error".to_string());
        assert_eq!(format!("{}", err), "cursor error: test cursor error");
    }

    #[test]
    fn test_sync_error_display_parse() {
        let err = SyncError::ParseError("bad format".to_string());
        assert_eq!(format!("{}", err), "parse error: bad format");
    }

    #[test]
    fn test_sync_error_display_decryption_failed() {
        let err = SyncError::DecryptionFailed {
            file_id: 42,
            source: crypto::error::CryptoError::MacMismatch,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("42"));
    }

    #[test]
    fn test_sync_error_debug_output() {
        let err = SyncError::Offline;
        let debug = format!("{:?}", err);
        assert!(debug.contains("Offline"));
    }
}
