use base64::Engine;
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;
use zoo_client::ZooClient;

const TEST_DB: &str = "zoo_client_test";
const TEST_BUCKET: &str = "test-bucket";
const S3_ENDPOINT: &str = "http://localhost:4566";
const S3_REGION: &str = "us-east-1";
const S3_ACCESS_KEY: &str = "test";
const S3_SECRET_KEY: &str = "test";

fn test_db_url() -> String {
    format!("postgres://postgres:postgres@localhost/{}", TEST_DB)
}

async fn ensure_test_db() {
    let default_url = "postgres://postgres:postgres@localhost/postgres";
    let default_pool = sqlx::PgPool::connect(default_url)
        .await
        .expect("failed to connect to default postgres database");

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(TEST_DB)
            .fetch_one(&default_pool)
            .await
            .expect("failed to check database existence");

    if !exists {
        let _ = sqlx::query(&format!("CREATE DATABASE {}", TEST_DB))
            .execute(&default_pool)
            .await;
    }

    default_pool.close().await;

    let test_pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect to test database");

    sqlx::migrate!("../zoo/migrations")
        .run(&test_pool)
        .await
        .expect("failed to run migrations");

    test_pool.close().await;
}

async fn clean_test_db() {
    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect to test database for cleanup");

    let tables = [
        "shares",
        "files",
        "upload_parts",
        "uploads",
        "devices",
        "sessions",
        "users",
    ];
    for table in &tables {
        let _ = sqlx::query(&format!("TRUNCATE {} CASCADE", table))
            .execute(&pool)
            .await;
    }

    pool.close().await;
}

struct TestServer {
    port: u16,
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
    _handle: std::thread::JoinHandle<()>,
}

impl TestServer {
    fn new(port: u16) -> Self {
        let addr = format!("127.0.0.1:{}", port);
        let db_url = test_db_url();
        let s3_endpoint = S3_ENDPOINT.to_string();
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
            rt.block_on(async {
                let mut config = zoo::config::ZooConfig::from_env();
                config.listen_addr = addr.clone();
                config.database_url = db_url;
                config.s3_endpoint = Some(s3_endpoint);
                config.s3_region = S3_REGION.to_string();
                config.s3_bucket = TEST_BUCKET.to_string();
                config.s3_access_key = S3_ACCESS_KEY.to_string();
                config.s3_secret_key = S3_SECRET_KEY.to_string();

                let db_url = config.database_url.clone();
                let app = zoo::create_app(&db_url, config)
                    .await
                    .expect("failed to create app");

                let listener = tokio::net::TcpListener::bind(&addr)
                    .await
                    .expect("failed to bind");

                let server = axum::serve(listener, app);
                tokio::select! {
                    _ = server => {},
                    _ = tokio::task::spawn_blocking(move || { shutdown_rx.recv().ok() }) => {},
                }
            });
        });

        Self {
            port,
            shutdown_tx: Some(shutdown_tx),
            _handle: handle,
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn wait_for_ready(&self, max_retries: usize) {
        let client = Client::new();
        for i in 0..max_retries {
            if client.get(&self.base_url()).send().await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(500 * (i as u64 + 1))).await;
        }
        panic!("server did not start within timeout");
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn register_and_login(client: &Client, base_url: &str, email: &str) -> String {
    let verify_key_plaintext = format!("{:x}", Sha256::digest(b"test_verify_key"));
    let verify_key_hash = bcrypt::hash(&verify_key_plaintext, bcrypt::DEFAULT_COST).unwrap();

    let kek_salt = base64::prelude::BASE64_STANDARD.encode([0u8; 16]);
    let encrypted_master_key = base64::prelude::BASE64_STANDARD.encode([0u8; 32]);
    let key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let public_key = base64::prelude::BASE64_STANDARD.encode([0u8; 32]);
    let encrypted_secret_key = base64::prelude::BASE64_STANDARD.encode([0u8; 48]);
    let secret_key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let encrypted_recovery_key = base64::prelude::BASE64_STANDARD.encode([0u8; 48]);
    let recovery_key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);

    let resp = client
        .post(&format!("{}/api/auth/register", base_url))
        .json(&json!({
            "email": email,
            "verify_key_hash": verify_key_hash,
            "encrypted_master_key": encrypted_master_key,
            "key_nonce": key_nonce,
            "kek_salt": kek_salt,
            "mem_limit": 67108864,
            "ops_limit": 2,
            "public_key": public_key,
            "encrypted_secret_key": encrypted_secret_key,
            "secret_key_nonce": secret_key_nonce,
            "encrypted_recovery_key": encrypted_recovery_key,
            "recovery_key_nonce": recovery_key_nonce,
        }))
        .send()
        .await
        .expect("register failed");

    let reg_status = resp.status();
    let reg_body = resp.text().await.expect("register response not text");
    eprintln!("register response: {} {}", reg_status, reg_body);

    let resp = client
        .post(&format!("{}/api/auth/login", base_url))
        .json(&json!({
            "email": email,
            "verify_key_hash": verify_key_plaintext,
        }))
        .send()
        .await
        .expect("login failed");

    let status = resp.status();
    let body_text = resp.text().await.expect("login response not text");
    let body: serde_json::Value = serde_json::from_str(&body_text).expect("login response not json");
    body["session_token"]
        .as_str()
        .unwrap_or_else(|| panic!("missing session_token in response (status {}): {}", status, body_text))
        .to_string()
}

async fn register_device_req(client: &Client, base_url: &str, token: &str) -> serde_json::Value {
    let resp = client
        .post(&format!("{}/api/devices", base_url))
        .bearer_auth(token)
        .json(&json!({
            "name": format!("test-device-{}", Uuid::new_v4()),
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    resp.json().await.expect("device response not json")
}

#[tokio::test]
async fn test_zoo_client_set_session_token() {
    let client = ZooClient::new("http://localhost:3030".to_string());
    client.set_session_token("test-token".to_string());
    let token = client.session_token().await;
    assert_eq!(token, Some("test-token".to_string()));
}

#[tokio::test]
async fn test_zoo_client_pending_uploads_empty() {
    ensure_test_db().await;
    clean_test_db().await;

    let port = 3030;
    let server = TestServer::new(port);
    server.wait_for_ready(20).await;

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    let uploads = client
        .pending_uploads()
        .await
        .expect("pending_uploads failed");
    assert!(uploads.is_empty());
}

#[tokio::test]
async fn test_zoo_client_cancel_upload() {
    ensure_test_db().await;
    clean_test_db().await;

    let port = 3031;
    let server = TestServer::new(port);
    server.wait_for_ready(20).await;

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;
    let device = register_device_req(&http_client, &base_url, &token).await;
    let device_id = device["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let resp = http_client
        .post(&format!("{}/api/uploads", base_url))
        .bearer_auth(&token)
        .header("x-device-id", device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": file_data.len() as i64,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 1,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("create upload failed");

    let body: serde_json::Value = resp.json().await.expect("upload response not json");
    let upload_id = body["upload_id"].as_str().unwrap().to_string();

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    client
        .cancel_upload(Uuid::parse_str(&upload_id).unwrap())
        .await
        .expect("cancel_upload failed");
}

#[tokio::test]
async fn test_zoo_client_download_file_not_found() {
    ensure_test_db().await;
    clean_test_db().await;

    let port = 3032;
    let server = TestServer::new(port);
    server.wait_for_ready(20).await;

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    let result = client.download_file(99999).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_zoo_client_get_thumbnail_not_found() {
    ensure_test_db().await;
    clean_test_db().await;

    let port = 3033;
    let server = TestServer::new(port);
    server.wait_for_ready(20).await;

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    let result = client.get_thumbnail(99999).await;
    assert!(result.is_err());
}
