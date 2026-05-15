use zoo_client::error::ZooError;
use zoo_client::upload::{s3_put_part, s3_complete};
use zoo_client::download::{download_file, get_thumbnail};
use zoo_client::sse::SseClient;
use zoo_client::ZooClient;

#[tokio::test]
async fn test_zoo_client_new() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    assert_eq!(client.base_url(), "http://localhost:3000");
}

#[tokio::test]
async fn test_zoo_client_set_session_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    client.set_session_token("test-token".to_string());
    let token = client.session_token().await;
    assert_eq!(token, Some("test-token".to_string()));
}

#[tokio::test]
async fn test_zoo_client_no_token_returns_none() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let token = client.session_token().await;
    assert!(token.is_none());
}

#[tokio::test]
async fn test_zoo_client_pending_uploads_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.pending_uploads().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_zoo_client_cancel_upload_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.cancel_upload(uuid::Uuid::new_v4()).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_zoo_client_download_file_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.download_file(1).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_zoo_client_get_thumbnail_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.get_thumbnail(1).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_zoo_client_upload_file_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.upload_file(
        &[],
        "hash",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection",
    ).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_zoo_client_resume_upload_not_authenticated() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.resume_upload(uuid::Uuid::new_v4(), &[]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::NotAuthenticated => {}
        _ => panic!("expected NotAuthenticated"),
    }
}

#[tokio::test]
async fn test_s3_put_part_invalid_url() {
    let result = s3_put_part("http://invalid-host-12345/part", &[0u8; 10]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_s3_complete_invalid_url() {
    let result = s3_complete("http://invalid-host-12345/complete", &["etag1".to_string()]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_download_file_invalid_url() {
    let client = reqwest::Client::new();
    let result = download_file(
        "http://invalid-host-12345",
        "token",
        1,
        &client,
    ).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_thumbnail_invalid_url() {
    let client = reqwest::Client::new();
    let result = get_thumbnail(
        "http://invalid-host-12345",
        "token",
        1,
        &client,
    ).await;
    assert!(result.is_err());
}

#[test]
fn test_sse_client_new() {
    let client = SseClient::new("http://localhost:3000".to_string(), "token".to_string());
    drop(client);
}

#[test]
fn test_zoo_error_display() {
    let err = ZooError::S3Error("connection refused".to_string());
    assert_eq!(err.to_string(), "S3 error: connection refused");

    let err = ZooError::UploadAborted;
    assert_eq!(err.to_string(), "upload was aborted by GC or manual action");

    let err = ZooError::StateError("invalid transition".to_string());
    assert_eq!(err.to_string(), "invalid state transition: invalid transition");

    let err = ZooError::ParseError("invalid json".to_string());
    assert_eq!(err.to_string(), "parse error: invalid json");

    let err = ZooError::NotAuthenticated;
    assert_eq!(err.to_string(), "not authenticated");

    let err = ZooError::Conflict("duplicate".to_string());
    assert_eq!(err.to_string(), "conflict: duplicate");

    let err = ZooError::UploadNotFound("uuid".to_string());
    assert_eq!(err.to_string(), "upload not found: uuid");

    let err = ZooError::NetworkUnavailable;
    assert_eq!(err.to_string(), "network unavailable");
}
