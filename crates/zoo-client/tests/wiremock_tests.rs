use base64::Engine;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{bearer_token, method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zoo_client::download::{download_file, get_thumbnail};
use zoo_client::error::ZooError;
use zoo_client::upload::{s3_complete, s3_put_part};
use zoo_client::sse::parse_sse_event_for_test;
use zoo_client::ZooClient;

async fn setup_mock_client(server: &MockServer) -> ZooClient {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let zoo = ZooClient::with_client(server.uri(), client);
    zoo.set_session_token("test-token".to_string());
    zoo
}

fn make_upload_state(upload_id: &str, status: &str, part_count: u16, bitmask: &str) -> types::upload::UploadState {
    types::upload::UploadState {
        upload_id: upload_id.to_string(),
        user_id: Uuid::new_v4().to_string(),
        device_id: Uuid::new_v4().to_string(),
        status: serde_json::from_value(json!(status)).unwrap(),
        file_hash: "hash".to_string(),
        file_size: 100,
        mime_type: Some("application/octet-stream".to_string()),
        part_size: 5242880,
        part_count,
        parts_bitmask: bitmask.to_string(),
        object_key: None,
        upload_id_s3: None,
        complete_url: None,
        urls_expire_at: None,
        last_heartbeat_at: None,
        stalled_at: None,
        error_reason: None,
        created_at: 0,
        expires_at: 0,
        done_at: None,
    }
}

// ==================== orchestrator.rs: create_upload 409 ====================

#[tokio::test]
async fn test_create_upload_409_conflict_with_id() {
    let server = MockServer::start().await;
    let existing_uuid = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "upload_id": existing_uuid.to_string()
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::Conflict(msg) => assert!(msg.contains(&existing_uuid.to_string())),
        e => panic!("expected Conflict, got {:?}", e),
    }
}

#[tokio::test]
async fn test_create_upload_409_conflict_no_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": "duplicate"
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::Conflict(msg) => assert!(msg.contains("no ID returned")),
        e => panic!("expected Conflict, got {:?}", e),
    }
}

#[tokio::test]
async fn test_create_upload_missing_upload_id_in_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "something": "else"
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("missing upload_id")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: patch_upload error ====================

#[tokio::test]
async fn test_patch_upload_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"internal error"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::StateError(msg) => assert!(msg.contains("patch upload failed")),
        e => panic!("expected StateError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: presign_urls error ====================

#[tokio::test]
async fn test_presign_urls_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign", upload_id)))
        .respond_with(ResponseTemplate::new(400).set_body_bytes(b"bad request"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("presign failed")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: presign_refresh error ====================

#[tokio::test]
async fn test_presign_refresh_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string(),
            "status": "uploading",
            "part_count": 1,
            "part_size": 5242880,
            "parts_bitmask": "",
            "file_hash": "hash123",
            "file_size": 100,
            "mime_type": "application/octet-stream",
            "device_id": Uuid::new_v4().to_string(),
            "user_id": Uuid::new_v4().to_string(),
            "created_at": 0,
            "expires_at": 0,
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign-refresh", upload_id)))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"server error"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.resume_upload(upload_id, &[]).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("presign-refresh failed")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: get_upload error ====================

#[tokio::test]
async fn test_get_upload_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(404).set_body_bytes(b"not found"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.resume_upload(upload_id, &[]).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("get upload failed")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: resume_upload Failed state ====================

#[tokio::test]
async fn test_resume_upload_failed_state() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string(),
            "status": "failed",
            "part_count": 1,
            "part_size": 5242880,
            "parts_bitmask": "",
            "file_hash": "hash123",
            "file_size": 100,
            "mime_type": "application/octet-stream",
            "device_id": Uuid::new_v4().to_string(),
            "user_id": Uuid::new_v4().to_string(),
            "created_at": 0,
            "expires_at": 0,
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.resume_upload(upload_id, &[]).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::UploadAborted => {}
        e => panic!("expected UploadAborted, got {:?}", e),
    }
}

// ==================== orchestrator.rs: register_upload error ====================

#[tokio::test]
async fn test_register_upload_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();
    let s3_part_url = format!("{}/s3/part1", server.uri());
    let s3_complete_url = format!("{}/s3/complete", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [s3_part_url],
            "complete_url": s3_complete_url
        })))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/s3/part1"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"abc123\""))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/s3/complete"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/register", upload_id)))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"register failed"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("register upload failed")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

#[tokio::test]
async fn test_register_upload_missing_file_id() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();
    let s3_part_url = format!("{}/s3/part1", server.uri());
    let s3_complete_url = format!("{}/s3/complete", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [s3_part_url],
            "complete_url": s3_complete_url
        })))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/s3/part1"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"abc123\""))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/s3/complete"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/register", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "something": "else"
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("missing file_id")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: cancel_upload error ====================

#[tokio::test]
async fn test_cancel_upload_error() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();

    Mock::given(method("DELETE"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(403).set_body_bytes(b"forbidden"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.cancel_upload(upload_id).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::StateError(msg) => assert!(msg.contains("cancel upload failed")),
        e => panic!("expected StateError, got {:?}", e),
    }
}

// ==================== orchestrator.rs: pending_uploads 404 ====================

#[tokio::test]
async fn test_pending_uploads_404_returns_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.pending_uploads().await.unwrap();
    assert!(result.is_empty());
}

// ==================== orchestrator.rs: find_missing_parts ====================

#[tokio::test]
async fn test_find_missing_parts_empty_bitmask() {
    let server = MockServer::start().await;
    let client = setup_mock_client(&server).await;

    let state = make_upload_state("test-uuid", "uploading", 3, "");
    let missing = client.find_missing_parts_for_test(&state, 100).unwrap();
    assert_eq!(missing, vec![0, 1, 2]);
}

#[tokio::test]
async fn test_find_missing_parts_partial_bitmask() {
    let server = MockServer::start().await;
    let client = setup_mock_client(&server).await;

    let bitmask = base64::prelude::BASE64_STANDARD.encode(vec![0b00000101]);
    let state = make_upload_state("test-uuid", "uploading", 4, &bitmask);
    let missing = client.find_missing_parts_for_test(&state, 100).unwrap();
    assert_eq!(missing, vec![1, 3]);
}

#[tokio::test]
async fn test_find_missing_parts_all_uploaded() {
    let server = MockServer::start().await;
    let client = setup_mock_client(&server).await;

    let bitmask = base64::prelude::BASE64_STANDARD.encode(vec![0b00000111]);
    let state = make_upload_state("test-uuid", "uploading", 3, &bitmask);
    let missing = client.find_missing_parts_for_test(&state, 100).unwrap();
    assert!(missing.is_empty());
}

// ==================== orchestrator.rs: upload_missing_parts out of range ====================

#[tokio::test]
async fn test_upload_missing_parts_out_of_range() {
    let server = MockServer::start().await;
    let client = setup_mock_client(&server).await;

    let upload_id = Uuid::new_v4().to_string();
    let state = make_upload_state(&upload_id, "uploading", 5, "");
    let missing = client.find_missing_parts_for_test(&state, 100).unwrap();
    assert_eq!(missing, vec![0, 1, 2, 3, 4]);

    let urls: Vec<String> = vec![];
    let result: Result<Vec<String>, ZooError> = client.upload_missing_parts_for_test(
        &upload_id.parse().unwrap(),
        &[],
        &urls,
        &missing,
        5242880,
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("out of range")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

// ==================== download.rs: redirect with Location ====================

#[tokio::test]
async fn test_download_file_redirect() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/download"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", format!("{}/direct", server.uri())))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/direct"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file content"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = download_file(&server.uri(), "token", 1, &client).await.unwrap();
    assert_eq!(result, b"file content");
}

#[tokio::test]
async fn test_download_file_redirect_missing_location() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/download"))
        .respond_with(ResponseTemplate::new(302))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = download_file(&server.uri(), "token", 1, &client).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("Location")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

#[tokio::test]
async fn test_download_file_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/download"))
        .respond_with(ResponseTemplate::new(404).set_body_bytes(b"not found"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = download_file(&server.uri(), "token", 1, &client).await;
    assert!(result.is_err());
}

// ==================== download.rs: get_thumbnail error ====================

#[tokio::test]
async fn test_get_thumbnail_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/thumbnail"))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"error"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = get_thumbnail(&server.uri(), "token", 1, &client).await;
    assert!(result.is_err());
}

// ==================== upload.rs: s3_put_part error ====================

#[tokio::test]
async fn test_s3_put_part_error_status() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"S3 error"))
        .mount(&server)
        .await;

    let result = s3_put_part(&server.uri(), &[0u8; 10]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::S3Error(msg) => assert!(msg.contains("S3 upload part failed")),
        e => panic!("expected S3Error, got {:?}", e),
    }
}

#[tokio::test]
async fn test_s3_put_part_missing_etag() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let result = s3_put_part(&server.uri(), &[0u8; 10]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::S3Error(msg) => assert!(msg.contains("missing ETag")),
        e => panic!("expected S3Error, got {:?}", e),
    }
}

// ==================== upload.rs: s3_complete error ====================

#[tokio::test]
async fn test_s3_complete_error_status() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_bytes(b"S3 complete error"))
        .mount(&server)
        .await;

    let result = s3_complete(&server.uri(), &["etag1".to_string()]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::S3Error(msg) => assert!(msg.contains("S3 complete multipart failed")),
        e => panic!("expected S3Error, got {:?}", e),
    }
}

// ==================== sse.rs: parse_sse_event ====================

#[test]
fn test_sse_parse_event_no_data() {
    let result = parse_sse_event_for_test("event: something\n\n");
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("no data field")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

#[test]
fn test_sse_parse_event_invalid_json() {
    let result = parse_sse_event_for_test("data: not valid json");
    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::ParseError(msg) => assert!(msg.contains("invalid SSE event")),
        e => panic!("expected ParseError, got {:?}", e),
    }
}

#[test]
fn test_sse_parse_event_valid_heartbeat() {
    let result = parse_sse_event_for_test("data: {\"type\":\"heartbeat\",\"timestamp\":1000}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_valid_device_connected() {
    let result = parse_sse_event_for_test("data: {\"type\":\"device_connected\",\"device_id\":\"dev1\",\"device_name\":\"Device 1\"}");
    assert!(result.is_ok());
}

// ==================== orchestrator.rs: upload_parts_with_heartbeat (5+ parts triggers heartbeat) ====================
// Note: The heartbeat logic is exercised in the full integration tests with real S3.
// The wiremock test below tests the 403 retry path which exercises the same code path.

// ==================== orchestrator.rs: upload_part_with_retry 403 then success ====================

#[tokio::test]
async fn test_upload_file_s3_403_retry() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();
    let s3_part_url = format!("{}/s3/part1", server.uri());
    let s3_complete_url = format!("{}/s3/complete", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [s3_part_url],
            "complete_url": s3_complete_url
        })))
        .mount(&server)
        .await;

    let mut seq = wiremock::Mock::given(method("PUT"))
        .and(path("/s3/part1"))
        .respond_with(ResponseTemplate::new(403).set_body_bytes(b"forbidden"))
        .up_to_n_times(1)
        .named("first_403");
    seq.mount(&server).await;

    Mock::given(method("PUT"))
        .and(path("/s3/part1"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"abc123\""))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/s3/complete"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/register", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": 456
        })))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 456);
}

// ==================== orchestrator.rs: upload_part_with_retry 403 exhausted ====================

#[tokio::test]
async fn test_upload_file_s3_403_retry_exhausted() {
    let server = MockServer::start().await;
    let upload_id = Uuid::new_v4();
    let s3_part_url = format!("{}/s3/part1", server.uri());
    let s3_complete_url = format!("{}/s3/complete", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "upload_id": upload_id.to_string()
        })))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/api/uploads/{}/presign", upload_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [s3_part_url],
            "complete_url": s3_complete_url
        })))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/s3/part1"))
        .respond_with(ResponseTemplate::new(403).set_body_bytes(b"forbidden"))
        .mount(&server)
        .await;

    let client = setup_mock_client(&server).await;
    let result = client.upload_file(
        &[],
        "hash123",
        vec!["d41d8cd98f00b204e9800998ecf8427e".to_string()],
        0,
        "application/octet-stream",
        "collection-1",
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ZooError::S3Error(msg) => assert!(msg.contains("403")),
        e => panic!("expected S3Error with 403, got {:?}", e),
    }
}

// ==================== orchestrator.rs: upload_missing_parts with heartbeat ====================

#[tokio::test]
async fn test_upload_missing_parts_with_success() {
    let server = MockServer::start().await;
    let client = setup_mock_client(&server).await;

    let upload_id = Uuid::new_v4();
    let s3_url = format!("{}/s3/part0", server.uri());

    Mock::given(method("PUT"))
        .and(path("/s3/part0"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag0\""))
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(format!("/api/uploads/{}", upload_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let state = make_upload_state(&upload_id.to_string(), "uploading", 1, "");
    let missing = client.find_missing_parts_for_test(&state, 100).unwrap();
    assert_eq!(missing, vec![0]);

    let urls = vec![s3_url];
    let result: Result<Vec<String>, ZooError> = client.upload_missing_parts_for_test(
        &upload_id,
        &[0u8; 10],
        &urls,
        &missing,
        5242880,
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec!["etag0"]);
}

// ==================== download.rs: successful direct download ====================

#[tokio::test]
async fn test_download_file_direct() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"direct content"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = download_file(&server.uri(), "token", 1, &client).await.unwrap();
    assert_eq!(result, b"direct content");
}

#[tokio::test]
async fn test_get_thumbnail_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/files/1/thumbnail"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"thumbnail data"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = get_thumbnail(&server.uri(), "token", 1, &client).await.unwrap();
    assert_eq!(result, b"thumbnail data");
}

// ==================== upload.rs: s3_put_part success ====================

#[tokio::test]
async fn test_s3_put_part_success() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"my-etag\""))
        .mount(&server)
        .await;

    let result = s3_put_part(&server.uri(), &[0u8; 10]).await.unwrap();
    assert_eq!(result, "my-etag");
}

#[tokio::test]
async fn test_s3_complete_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let result = s3_complete(&server.uri(), &["etag1".to_string(), "etag2".to_string()]).await;
    assert!(result.is_ok());
}

// ==================== sse.rs: more parse tests ====================

#[test]
fn test_sse_parse_event_upload_progress() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_progress\",\"upload_id\":\"up1\",\"status\":\"uploading\",\"parts_bitmask\":\"AA\",\"part_count\":1,\"device_name\":\"Dev1\"}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_upload_done() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_done\",\"upload_id\":\"up1\",\"file_id\":123,\"device_name\":\"Dev1\"}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_upload_failed() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_failed\",\"upload_id\":\"up1\",\"reason\":\"error\",\"device_name\":\"Dev1\"}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_upload_stalled() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_stalled\",\"upload_id\":\"up1\",\"parts_bitmask\":\"AA\",\"part_count\":1,\"device_name\":\"Dev1\",\"stalled_at\":1000}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_upload_completed() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_completed\",\"upload_id\":\"up1\",\"device_name\":\"Dev1\"}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_upload_pending() {
    let result = parse_sse_event_for_test("data: {\"type\":\"upload_pending\",\"uploads\":[]}");
    assert!(result.is_ok());
}

#[test]
fn test_sse_parse_event_device_disconnected() {
    let result = parse_sse_event_for_test("data: {\"type\":\"device_disconnected\",\"device_id\":\"dev1\",\"device_name\":\"Dev1\"}");
    assert!(result.is_ok());
}

