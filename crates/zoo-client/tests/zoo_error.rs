use zoo_client::ZooError;

#[test]
fn test_zoo_error_display_http() {
    let client = reqwest::Client::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt.block_on(async {
        let resp = client.get("http://localhost:1").send().await.unwrap_err();
        ZooError::HttpError(resp)
    });
    let msg = format!("{}", err);
    assert!(msg.contains("HTTP error"));
}

#[test]
fn test_zoo_error_display_s3() {
    let err = ZooError::S3Error("bucket not found".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("S3 error"));
    assert!(msg.contains("bucket not found"));
}

#[test]
fn test_zoo_error_display_upload_aborted() {
    let err = ZooError::UploadAborted;
    let msg = format!("{}", err);
    assert!(msg.contains("upload was aborted"));
}

#[test]
fn test_zoo_error_display_state() {
    let err = ZooError::StateError("invalid transition".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("invalid state transition"));
    assert!(msg.contains("invalid transition"));
}

#[test]
fn test_zoo_error_display_parse() {
    let err = ZooError::ParseError("bad json".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("parse error"));
    assert!(msg.contains("bad json"));
}

#[test]
fn test_zoo_error_display_not_authenticated() {
    let err = ZooError::NotAuthenticated;
    let msg = format!("{}", err);
    assert!(msg.contains("not authenticated"));
}

#[test]
fn test_zoo_error_display_conflict() {
    let err = ZooError::Conflict("device name taken".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("conflict"));
    assert!(msg.contains("device name taken"));
}

#[test]
fn test_zoo_error_display_upload_not_found() {
    let err = ZooError::UploadNotFound("abc".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("upload not found"));
    assert!(msg.contains("abc"));
}

#[test]
fn test_zoo_error_display_network_unavailable() {
    let err = ZooError::NetworkUnavailable;
    let msg = format!("{}", err);
    assert!(msg.contains("network unavailable"));
}

#[test]
fn test_zoo_error_debug() {
    let err = ZooError::NotAuthenticated;
    let debug = format!("{:?}", err);
    assert!(debug.contains("NotAuthenticated"));
}
