use zoo_client::sse::SseClient;
use zoo_client::ZooClient;

#[test]
fn test_sse_client_new() {
    let client = SseClient::new(
        "http://localhost:3000".to_string(),
        "test-token".to_string(),
    );
    // SseClient fields are private, but construction should succeed
    let _ = client;
}

#[test]
fn test_sse_client_new_with_custom_token() {
    let client = SseClient::new(
        "https://api.example.com".to_string(),
        "bearer-abc-123".to_string(),
    );
    let _ = client;
}

#[test]
fn test_zoo_client_new() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    assert_eq!(client.base_url(), "http://localhost:3000");
}

#[test]
fn test_zoo_client_new_with_https() {
    let client = ZooClient::new("https://api.example.com".to_string());
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[tokio::test]
async fn test_zoo_client_set_and_get_session_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    assert!(client.session_token().await.is_none());

    client.set_session_token("my-token".to_string());
    assert_eq!(client.session_token().await, Some("my-token".to_string()));
}

#[tokio::test]
async fn test_zoo_client_overwrite_session_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    client.set_session_token("token-1".to_string());
    assert_eq!(client.session_token().await, Some("token-1".to_string()));

    client.set_session_token("token-2".to_string());
    assert_eq!(client.session_token().await, Some("token-2".to_string()));
}

#[tokio::test]
async fn test_zoo_client_base_url() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    assert_eq!(client.base_url(), "http://localhost:3000");
}

#[tokio::test]
async fn test_zoo_client_pending_uploads_without_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.pending_uploads().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_zoo_client_cancel_upload_without_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.cancel_upload(uuid::Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_zoo_client_download_file_without_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.download_file(1).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_zoo_client_get_thumbnail_without_token() {
    let client = ZooClient::new("http://localhost:3000".to_string());
    let result = client.get_thumbnail(1).await;
    assert!(result.is_err());
}
