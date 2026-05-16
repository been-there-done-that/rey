use sync::SyncError;

#[test]
fn test_sync_error_display_network() {
    let err = SyncError::NetworkError(zoo_client::ZooError::NotAuthenticated);
    let msg = format!("{}", err);
    assert!(msg.contains("network error"));
    assert!(msg.contains("not authenticated"));
}

#[test]
fn test_sync_error_display_decryption() {
    let crypto_err = crypto::error::CryptoError::MacMismatch;
    let err = SyncError::DecryptionFailed {
        file_id: 42,
        source: crypto_err,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("decryption failed"));
    assert!(msg.contains("42"));
}

#[test]
fn test_sync_error_display_cursor() {
    let err = SyncError::CursorError("test error".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("cursor error"));
    assert!(msg.contains("test error"));
}

#[test]
fn test_sync_error_display_offline() {
    let err = SyncError::Offline;
    let msg = format!("{}", err);
    assert!(msg.contains("offline mode"));
}

#[test]
fn test_sync_error_display_parse() {
    let err = SyncError::ParseError("bad data".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("parse error"));
    assert!(msg.contains("bad data"));
}

#[test]
fn test_sync_error_debug() {
    let err = SyncError::Offline;
    let debug = format!("{:?}", err);
    assert!(debug.contains("Offline"));
}
