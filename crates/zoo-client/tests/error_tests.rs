use zoo_client::error::ZooError;

#[test]
fn test_zoo_error_s3_error() {
    let err = ZooError::S3Error("access denied".to_string());
    assert_eq!(err.to_string(), "S3 error: access denied");
}

#[test]
fn test_zoo_error_upload_aborted() {
    let err = ZooError::UploadAborted;
    assert_eq!(err.to_string(), "upload was aborted by GC or manual action");
}

#[test]
fn test_zoo_error_state_error() {
    let err = ZooError::StateError("invalid transition".to_string());
    assert_eq!(err.to_string(), "invalid state transition: invalid transition");
}

#[test]
fn test_zoo_error_parse_error() {
    let err = ZooError::ParseError("invalid json".to_string());
    assert_eq!(err.to_string(), "parse error: invalid json");
}

#[test]
fn test_zoo_error_not_authenticated() {
    let err = ZooError::NotAuthenticated;
    assert_eq!(err.to_string(), "not authenticated");
}

#[test]
fn test_zoo_error_conflict() {
    let err = ZooError::Conflict("duplicate upload".to_string());
    assert_eq!(err.to_string(), "conflict: duplicate upload");
}

#[test]
fn test_zoo_error_upload_not_found() {
    let err = ZooError::UploadNotFound("uuid-123".to_string());
    assert_eq!(err.to_string(), "upload not found: uuid-123");
}

#[test]
fn test_zoo_error_network_unavailable() {
    let err = ZooError::NetworkUnavailable;
    assert_eq!(err.to_string(), "network unavailable");
}
