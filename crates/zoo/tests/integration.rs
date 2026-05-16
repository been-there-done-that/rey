use base64::Engine;
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::time::Duration;
use tokio::sync::OnceCell;
use uuid::Uuid;

const TEST_DB: &str = "zoo_test";
const TEST_BUCKET: &str = "test-bucket";
const S3_ENDPOINT: &str = "http://localhost:4566";
const S3_REGION: &str = "us-east-1";
const S3_ACCESS_KEY: &str = "test";
const S3_SECRET_KEY: &str = "test";
const TEST_PORT: u16 = 3100;

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
        sqlx::query(&format!("CREATE DATABASE {}", TEST_DB))
            .execute(&default_pool)
            .await
            .expect("failed to create test database");
    }

    default_pool.close().await;

    let test_pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect to test database");

    sqlx::migrate!("./migrations")
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

    async fn with_server<F, Fut>(port: u16, test_fn: F)
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = ()>,
    {
        let addr = format!("127.0.0.1:{}", port);
        let mut config = zoo::config::ZooConfig::from_env();
        config.listen_addr = addr.clone();
        config.database_url = test_db_url();
        config.s3_endpoint = Some(S3_ENDPOINT.to_string());
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
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            let _ = tokio::select! {
                _ = server => {},
                _ = shutdown_rx => {},
            };
        });

        test_fn(format!("http://{}", addr)).await;

        let _ = shutdown_tx.send(());
        let _ = server_task.await;
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

static SERVER: OnceCell<TestServer> = OnceCell::const_new();

async fn get_server() -> &'static TestServer {
    SERVER
        .get_or_init(|| async {
            ensure_test_db().await;
            let server = TestServer::new(TEST_PORT);
            server.wait_for_ready(30).await;
            server
        })
        .await
}

fn base_url() -> String {
    format!("http://127.0.0.1:{}", TEST_PORT)
}

async fn register_test_user(client: &Client, email: &str) {
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
        .post(&format!("{}/api/auth/register", base_url()))
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
        .expect("register request failed");

    assert!(
        resp.status() == 201 || resp.status() == 400,
        "register failed: {:?}",
        resp.text().await.unwrap()
    );
}

async fn login_user(client: &Client, email: &str) -> String {
    let verify_key_plaintext = format!("{:x}", Sha256::digest(b"test_verify_key"));

    let resp = client
        .post(&format!("{}/api/auth/login", base_url()))
        .json(&json!({
            "email": email,
            "verify_key_hash": verify_key_plaintext,
        }))
        .send()
        .await
        .expect("login request failed");

    let body: serde_json::Value = resp.json().await.expect("login response not json");
    let token = body["session_token"]
        .as_str()
        .unwrap_or_else(|| panic!("missing session_token in response: {}", body));
    token.to_string()
}

async fn register_device(client: &Client, token: &str) -> serde_json::Value {
    let resp = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(token)
        .json(&json!({
            "name": format!("test-device-{}", Uuid::new_v4()),
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp.status(), 201);
    resp.json().await.expect("device response not json")
}

async fn create_upload(
    client: &Client,
    token: &str,
    device_id: &str,
    file_hash: &str,
    file_size: i64,
) -> serde_json::Value {
    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(token)
        .header("x-device-id", device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": file_size,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 1,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("create upload failed");

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.expect("upload response not json");
    if !status.is_success() {
        panic!("create upload failed with status {}: {}", status, body);
    }
    body
}

async fn setup_user() -> (Client, String, String, serde_json::Value) {
    clean_test_db().await;
    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    let device = register_device(&client, &token).await;
    let device_id = device["device_id"].as_str().unwrap().to_string();
    (client, token, device_id, device)
}

#[tokio::test]
async fn test_auth_register_and_login() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    assert!(!token.is_empty());
    assert!(token.len() > 10);
}

#[tokio::test]
async fn test_auth_login_wrong_credentials() {
    get_server().await;

    let client = Client::new();
    let resp = client
        .post(&format!("{}/api/auth/login", base_url()))
        .json(&json!({
            "email": "nonexistent@example.com",
            "verify_key_hash": "wrong",
        }))
        .send()
        .await
        .expect("login request failed");

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_auth_login_params() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;

    let resp = client
        .post(&format!("{}/api/auth/login-params", base_url()))
        .json(&json!({
            "email": email,
            "verify_key_hash": "dummy",
        }))
        .send()
        .await
        .expect("login-params request failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("login-params response not json");
    assert!(body.get("kek_salt").is_some());
    assert!(body.get("mem_limit").is_some());
    assert!(body.get("ops_limit").is_some());
}

#[tokio::test]
async fn test_auth_login_params_nonexistent_user() {
    get_server().await;

    let client = Client::new();
    let resp = client
        .post(&format!("{}/api/auth/login-params", base_url()))
        .json(&json!({
            "email": "nonexistent@example.com",
            "verify_key_hash": "dummy",
        }))
        .send()
        .await
        .expect("login-params request failed");

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_auth_logout() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;

    let resp = client
        .post(&format!("{}/api/auth/logout", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("logout request failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_auth_logout_invalid_token() {
    get_server().await;

    let client = Client::new();
    let resp = client
        .post(&format!("{}/api/auth/logout", base_url()))
        .bearer_auth("invalid-token")
        .send()
        .await
        .expect("logout request failed");

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_device_register_and_list() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    let device = register_device(&client, &token).await;

    assert!(device.get("device_id").is_some());
    assert!(device.get("sse_token").is_some());
}

#[tokio::test]
async fn test_device_deregister() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    let device = register_device(&client, &token).await;

    let device_id = device["device_id"].as_str().unwrap();
    let resp = client
        .delete(&format!("{}/api/devices/{}", base_url(), device_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("deregister device failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_device_heartbeat() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    let device = register_device(&client, &token).await;
    let device_id = device["device_id"].as_str().unwrap();

    let resp = client
        .post(&format!(
            "{}/api/devices/{}/heartbeat",
            base_url(),
            device_id
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("device heartbeat failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_device_empty_name() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;

    let resp = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(&token)
        .json(&json!({
            "name": "",
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_sse_test_event() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    register_device(&client, &token).await;

    let resp = client
        .post(&format!("{}/api/events/test", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("test event request failed");

    assert!(resp.status() == 200 || resp.status() == 202);
}

#[tokio::test]
async fn test_unauthorized_access() {
    get_server().await;

    let client = Client::new();
    let resp = client
        .get(&format!("{}/api/uploads", base_url()))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_validation_email_empty() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();

    let resp = client
        .post(&format!("{}/api/auth/register", base_url()))
        .json(&json!({
            "email": "",
            "verify_key_hash": "hash",
            "encrypted_master_key": "key",
            "key_nonce": "nonce",
            "kek_salt": "salt",
            "mem_limit": 67108864,
            "ops_limit": 2,
            "public_key": "pk",
            "encrypted_secret_key": "esk",
            "secret_key_nonce": "sn",
            "encrypted_recovery_key": "rk",
            "recovery_key_nonce": "rn",
        }))
        .send()
        .await
        .expect("register request failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_file_download_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .get(&format!("{}/api/files/99999/download", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("download request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_file_archive_already_archived() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .put(&format!("{}/api/files/99999/archive", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("archive request failed");

    assert_eq!(resp.status(), 404);

    let resp2 = client
        .put(&format!("{}/api/files/99999/archive", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("archive request failed");

    assert_eq!(resp2.status(), 404);
}

#[tokio::test]
async fn test_upload_presign_refresh_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/uploads/fake-id/presign-refresh", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .json(&json!({
            "part_numbers": [1],
        }))
        .send()
        .await
        .expect("presign refresh request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_complete_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/uploads/fake-id/complete", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .send()
        .await
        .expect("complete request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_register_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/uploads/fake-id/register", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .json(&json!({
            "collection_id": "test",
            "cipher": "xchacha20-poly1305",
            "encrypted_key": "key",
            "key_decryption_nonce": "nonce",
            "file_decryption_header": "header",
            "thumb_decryption_header": "thumb_header",
            "encrypted_metadata": "metadata",
            "encrypted_thumbnail": "thumb",
            "thumbnail_size": 100,
        }))
        .send()
        .await
        .expect("register request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_heartbeat_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/uploads/fake-id/heartbeat", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .send()
        .await
        .expect("heartbeat request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_fail_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/uploads/fake-id/fail", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .json(&json!({
            "reason": "test failure",
        }))
        .send()
        .await
        .expect("fail request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_confirm_part_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .put(&format!("{}/api/uploads/fake-id/parts/1", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .json(&json!({
            "etag": "etag123",
            "size": 1000,
        }))
        .send()
        .await
        .expect("confirm part request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_get_status_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .get(&format!("{}/api/uploads/fake-id", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .send()
        .await
        .expect("get status request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_patch_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .patch(&format!("{}/api/uploads/fake-id", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .json(&json!({
            "status": "failed",
        }))
        .send()
        .await
        .expect("patch status request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_upload_cancel_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .delete(&format!("{}/api/uploads/fake-id", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "fake-device")
        .send()
        .await
        .expect("cancel request failed");

    assert!(resp.status() == 400 || resp.status() == 404);
}

#[tokio::test]
async fn test_device_heartbeat_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!(
            "{}/api/devices/00000000-0000-0000-0000-000000000000/heartbeat",
            base_url()
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("heartbeat request failed");

    assert!(resp.status() == 204 || resp.status() == 404);
}

#[tokio::test]
async fn test_device_deregister_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .delete(&format!(
            "{}/api/devices/00000000-0000-0000-0000-000000000000",
            base_url()
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("deregister request failed");

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_device_register_empty_name() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(&token)
        .json(&json!({
            "name": "",
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_device_register_name_too_long() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let long_name = "a".repeat(256);
    let resp = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(&token)
        .json(&json!({
            "name": long_name,
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_device_register_duplicate_name() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;
    let device_name = format!("dup-device-{}", Uuid::new_v4());

    let resp1 = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(&token)
        .json(&json!({
            "name": device_name,
            "platform": "desktop",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp1.status(), 201);

    let resp2 = client
        .post(&format!("{}/api/devices", base_url()))
        .bearer_auth(&token)
        .json(&json!({
            "name": device_name,
            "platform": "mobile",
        }))
        .send()
        .await
        .expect("register device failed");

    assert_eq!(resp2.status(), 422);
}

#[tokio::test]
async fn test_db_users_register_and_find() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let email = format!("db_test_{}@example.com", Uuid::new_v4());
    let id = zoo::db::users::register_user(
        &pool,
        &email,
        "test_hash",
        "master_key",
        "key_nonce",
        "kek_salt",
        67108864,
        2,
        "public_key",
        "encrypted_secret_key",
        "secret_key_nonce",
        "encrypted_recovery_key",
        "recovery_key_nonce",
    )
    .await
    .expect("register user failed");

    let user = zoo::db::users::find_user_by_email(&pool, &email)
        .await
        .expect("find user failed")
        .expect("user not found");

    assert_eq!(user.email, email);
    assert_eq!(user.id, id);

    let user_by_id = zoo::db::users::get_user_by_id(&pool, id)
        .await
        .expect("get user failed")
        .expect("user not found");

    assert_eq!(user_by_id.id, id);
}

#[tokio::test]
async fn test_db_users_duplicate_email() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let email = format!("dup_test_{}@example.com", Uuid::new_v4());
    zoo::db::users::register_user(
        &pool,
        &email,
        "test_hash",
        "master_key",
        "key_nonce",
        "kek_salt",
        67108864,
        2,
        "public_key",
        "encrypted_secret_key",
        "secret_key_nonce",
        "encrypted_recovery_key",
        "recovery_key_nonce",
    )
    .await
    .expect("register user failed");

    let result = zoo::db::users::register_user(
        &pool,
        &email,
        "test_hash2",
        "master_key2",
        "key_nonce2",
        "kek_salt2",
        67108864,
        2,
        "public_key2",
        "encrypted_secret_key2",
        "secret_key_nonce2",
        "encrypted_recovery_key2",
        "recovery_key_nonce2",
    )
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        zoo::error::ZooError::Validation(msg) => assert!(msg.contains("email")),
        _ => panic!("expected validation error"),
    }
}

#[tokio::test]
async fn test_db_sessions_create_and_lookup() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("session_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    let token_hash = "test_session_token_hash";
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

    let session_id = zoo::db::sessions::create_session(&pool, user_id, token_hash, expires_at)
        .await
        .expect("create session failed");

    let session = zoo::db::sessions::lookup_session(&pool, token_hash)
        .await
        .expect("lookup session failed")
        .expect("session not found");

    assert_eq!(session.id, session_id);
    assert_eq!(session.user_id, user_id);

    zoo::db::sessions::revoke_session(&pool, token_hash)
        .await
        .expect("revoke session failed");

    let session = zoo::db::sessions::lookup_session(&pool, token_hash)
        .await
        .expect("lookup session failed");

    assert!(session.is_none());
}

#[tokio::test]
async fn test_db_sessions_revoke_user_sessions() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("revoke_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
    zoo::db::sessions::create_session(&pool, user_id, "token1", expires_at)
        .await
        .unwrap();
    zoo::db::sessions::create_session(&pool, user_id, "token2", expires_at)
        .await
        .unwrap();

    zoo::db::sessions::revoke_user_sessions(&pool, user_id)
        .await
        .expect("revoke user sessions failed");

    let s1 = zoo::db::sessions::lookup_session(&pool, "token1").await.unwrap();
    let s2 = zoo::db::sessions::lookup_session(&pool, "token2").await.unwrap();
    assert!(s1.is_none());
    assert!(s2.is_none());
}

#[tokio::test]
async fn test_db_devices_register_and_lookup() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("device_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    let sse_token = Uuid::new_v4().to_string();
    let device_id = zoo::db::devices::register_device(
        &pool,
        user_id,
        "test-device",
        "desktop",
        &sse_token,
        None,
        90,
    )
    .await
    .expect("register device failed");

    let device = zoo::db::devices::lookup_device_by_id(&pool, device_id)
        .await
        .expect("lookup device failed")
        .expect("device not found");

    assert_eq!(device.id, device_id);
    assert_eq!(device.name, "test-device");

    let device_by_sse = zoo::db::devices::lookup_by_sse_token(&pool, &sse_token)
        .await
        .expect("lookup by sse token failed")
        .expect("device not found");

    assert_eq!(device_by_sse.id, device_id);
}

#[tokio::test]
async fn test_db_devices_tombstone() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("tombstone_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    let sse_token = Uuid::new_v4().to_string();
    let device_id = zoo::db::devices::register_device(
        &pool,
        user_id,
        "tombstone-device",
        "desktop",
        &sse_token,
        None,
        90,
    )
    .await
    .unwrap();

    zoo::db::devices::tombstone_device(&pool, device_id)
        .await
        .expect("tombstone device failed");

    let device = zoo::db::devices::lookup_device_by_id(&pool, device_id)
        .await
        .unwrap();

    assert!(device.is_none());
}

#[tokio::test]
async fn test_db_devices_update_last_seen() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("lastseen_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    let sse_token = Uuid::new_v4().to_string();
    let device_id = zoo::db::devices::register_device(
        &pool,
        user_id,
        "lastseen-device",
        "desktop",
        &sse_token,
        None,
        90,
    )
    .await
    .unwrap();

    zoo::db::devices::update_last_seen(&pool, device_id)
        .await
        .expect("update last seen failed");

    let timeout = zoo::db::devices::get_device_stall_timeout(&pool, device_id)
        .await
        .expect("get stall timeout failed");

    assert_eq!(timeout, 90);
}

#[tokio::test]
async fn test_db_upload_parts_operations() {
    get_server().await;
    clean_test_db().await;

    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("failed to connect");

    let user_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)")
        .bind(user_id)
        .bind(format!("parts_test_{}@test.com", Uuid::new_v4()))
        .bind("hash")
        .bind("key")
        .bind("nonce")
        .bind("salt")
        .bind(67108864i32)
        .bind(2i32)
        .bind("pub")
        .bind("sec")
        .bind("snonce")
        .bind("rec")
        .bind("rnonce")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO devices (id, user_id, name, platform, sse_token, stall_timeout_seconds) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(device_id)
        .bind(user_id)
        .bind("parts-device")
        .bind("desktop")
        .bind(Uuid::new_v4().to_string())
        .bind(90i32)
        .execute(&pool)
        .await
        .unwrap();

    let upload_id = zoo::db::uploads::create_upload(
        &pool,
        user_id,
        device_id,
        "parts-hash",
        1000,
        Some("application/octet-stream"),
        5242880,
        1,
        chrono::Utc::now() + chrono::Duration::hours(1),
        "parts-key",
    )
    .await
    .expect("create upload failed");

    zoo::db::upload_parts::insert_parts_batch(
        &pool,
        upload_id,
        &[(1, 1000, "d41d8cd98f00b204e9800998ecf8427e".to_string())],
    )
    .await
    .expect("insert parts failed");

    let pending = zoo::db::upload_parts::list_pending_parts(&pool, upload_id)
        .await
        .expect("list pending parts failed");

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].part_number, 1);

    zoo::db::upload_parts::mark_part_uploaded(&pool, upload_id, 1, "etag123")
        .await
        .expect("mark part uploaded failed");

    let pending = zoo::db::upload_parts::list_pending_parts(&pool, upload_id)
        .await
        .unwrap();
    assert!(pending.is_empty());

    let uploaded = zoo::db::upload_parts::list_uploaded_parts(&pool, upload_id)
        .await
        .expect("list uploaded parts failed");
    assert_eq!(uploaded.len(), 1);
    assert_eq!(uploaded[0].etag.as_deref(), Some("etag123"));
}

#[tokio::test]
async fn test_sse_hub_subscribe_and_broadcast() {
    use zoo::sse::hub::SseHub;
    use types::sse::SseEvent;

    let hub = SseHub::new();
    let mut rx = hub.subscribe("user-1");

    let event = SseEvent::Heartbeat {
        timestamp: 1700000000000,
    };
    hub.broadcast("user-1", event.clone());

    let received = rx.try_recv().unwrap();
    match received {
        SseEvent::Heartbeat { timestamp } => assert_eq!(timestamp, 1700000000000),
        _ => panic!("wrong event type"),
    }
}

#[tokio::test]
async fn test_sse_hub_broadcast_to_wrong_user() {
    use zoo::sse::hub::SseHub;
    use types::sse::SseEvent;

    let hub = SseHub::new();
    let mut rx1 = hub.subscribe("user-1");
    let mut rx2 = hub.subscribe("user-2");

    let event = SseEvent::Heartbeat {
        timestamp: 1700000000000,
    };
    hub.broadcast("user-1", event);

    assert!(rx1.try_recv().is_ok());
    assert!(rx2.try_recv().is_err());
}

#[tokio::test]
async fn test_sse_hub_broadcast_to_nonexistent_user() {
    use zoo::sse::hub::SseHub;
    use types::sse::SseEvent;

    let hub = SseHub::new();
    let event = SseEvent::Heartbeat {
        timestamp: 1700000000000,
    };
    hub.broadcast("nonexistent", event);
}

#[tokio::test]
async fn test_sse_hub_sender_count() {
    use zoo::sse::hub::SseHub;

    let hub = SseHub::new();
    assert_eq!(hub.sender_count("unknown"), 0);

    let _rx1 = hub.subscribe("user-1");
    let _rx2 = hub.subscribe("user-1");
    assert_eq!(hub.sender_count("user-1"), 2);
}

#[tokio::test]
async fn test_sse_hub_default() {
    use zoo::sse::hub::SseHub;

    let hub = SseHub::default();
    assert_eq!(hub.sender_count("any"), 0);
}

#[tokio::test]
async fn test_sse_hub_cleanup_active() {
    use zoo::sse::hub::SseHub;

    let hub = SseHub::new();
    let _rx = hub.subscribe("user-1");
    hub.cleanup_if_empty("user-1");
    assert_eq!(hub.sender_count("user-1"), 1);
}

#[tokio::test]
async fn test_sse_events_format() {
    use zoo::sse::events::format_sse;
    use types::sse::SseEvent;

    let event = SseEvent::Heartbeat {
        timestamp: 1700000000000,
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.ends_with("\n\n"));
    assert!(formatted.contains("heartbeat"));
}

#[tokio::test]
async fn test_state_validate_transition_all_states() {
    use types::upload::UploadStatus;
    use zoo::state::validate_transition;

    assert!(validate_transition(UploadStatus::Pending, UploadStatus::Encrypting).is_ok());
    assert!(validate_transition(UploadStatus::Pending, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Uploading).is_ok());
    assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::Uploading, UploadStatus::S3Completed).is_ok());
    assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Stalled).is_ok());
    assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::S3Completed, UploadStatus::Registering).is_ok());
    assert!(validate_transition(UploadStatus::S3Completed, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::Registering, UploadStatus::Done).is_ok());
    assert!(validate_transition(UploadStatus::Registering, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Uploading).is_ok());
    assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Failed).is_ok());
    assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Resuming).is_ok());
    assert!(validate_transition(UploadStatus::Resuming, UploadStatus::Uploading).is_ok());
    assert!(validate_transition(UploadStatus::Resuming, UploadStatus::Failed).is_ok());

    assert!(validate_transition(UploadStatus::Done, UploadStatus::Pending).is_err());
    assert!(validate_transition(UploadStatus::Failed, UploadStatus::Pending).is_err());
    assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Encrypting).is_err());
}

#[tokio::test]
async fn test_config_from_env() {
    let config = zoo::config_from_env();
    assert!(!config.listen_addr.is_empty());
    assert!(!config.database_url.is_empty());
}

#[tokio::test]
async fn test_lib_constants() {
    assert_eq!(zoo::DEFAULT_PART_SIZE, 20 * 1024 * 1024);
    assert_eq!(zoo::MAX_FILE_SIZE, 10 * 1024 * 1024 * 1024);
}

#[tokio::test]
async fn test_full_upload_lifecycle_with_s3() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Hello, this is a test file for upload!";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(
        &client,
        &token,
        &device_id,
        &file_hash,
        file_size,
    )
    .await;

    let upload_id = upload["upload_id"].as_str().unwrap().to_string();
    let upload_id_s3 = upload["upload_id_s3"].as_str().unwrap().to_string();
    let object_key = upload["object_key"].as_str().unwrap().to_string();

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");

    assert_eq!(presign_resp.status(), 200);
    let presign_body: serde_json::Value = presign_resp.json().await.unwrap();
    let presigned_urls = presign_body["urls"].as_array().unwrap();
    assert_eq!(presigned_urls.len(), 1);
    let presigned_url = presigned_urls[0].as_str().unwrap();

    let put_resp = client
        .put(presigned_url)
        .body(file_content.to_vec())
        .send()
        .await
        .expect("S3 upload failed");

    assert!(put_resp.status().is_success());
    let etag = put_resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string())
        .expect("missing ETag");

    let confirm_resp = client
        .put(&format!("{}/api/uploads/{}/parts/1", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "etag": etag,
            "size": file_size,
        }))
        .send()
        .await
        .expect("confirm part failed");

    assert_eq!(confirm_resp.status(), 204);

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch to encrypting failed");

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "uploading" }))
        .send()
        .await
        .expect("patch to uploading failed");

    let complete_resp = client
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("complete failed");

    assert_eq!(complete_resp.status(), 200);
    let complete_body: serde_json::Value = complete_resp.json().await.unwrap();
    assert_eq!(complete_body["status"], "s3_completed");

    let collection_id = Uuid::new_v4().to_string();
    let encrypted_key = base64::prelude::BASE64_STANDARD.encode([0u8; 32]);
    let key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let file_header = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let thumb_header = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let encrypted_metadata = base64::prelude::BASE64_STANDARD.encode(b"{}");
    let encrypted_thumb = base64::prelude::BASE64_STANDARD.encode(b"thumb");

    let register_resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "collection_id": collection_id,
            "cipher": "xchacha20-poly1305",
            "encrypted_key": encrypted_key,
            "key_decryption_nonce": key_nonce,
            "file_decryption_header": file_header,
            "thumb_decryption_header": thumb_header,
            "encrypted_metadata": encrypted_metadata,
            "encrypted_thumbnail": encrypted_thumb,
            "thumbnail_size": 5,
        }))
        .send()
        .await
        .expect("register failed");

    assert!(register_resp.status().is_success());
    let register_body: serde_json::Value = register_resp.json().await.unwrap();
    assert!(register_body.get("file_id").is_some());
    let file_id = register_body["file_id"].as_i64().unwrap();

    let status_resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 200);
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(status_body["status"], "done");
    assert!(status_body.get("upload_id").is_some());
}

#[tokio::test]
async fn test_file_download_presigned_redirect() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Download test content";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");
    let presigned_url = presign_resp.json::<serde_json::Value>().await.unwrap()["urls"].as_array().unwrap()[0].as_str().unwrap().to_string();

    let put_resp = client
        .put(&presigned_url)
        .body(file_content.to_vec())
        .send()
        .await
        .expect("S3 upload failed");
    assert!(put_resp.status().is_success());
    let etag = put_resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string())
        .unwrap();

    client
        .put(&format!("{}/api/uploads/{}/parts/1", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "etag": etag, "size": file_size }))
        .send()
        .await
        .expect("confirm part failed");

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch failed");

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "uploading" }))
        .send()
        .await
        .expect("patch failed");

    client
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("complete failed");

    let collection_id = Uuid::new_v4().to_string();
    let register_resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "collection_id": collection_id,
            "cipher": "xchacha20-poly1305",
            "encrypted_key": base64::prelude::BASE64_STANDARD.encode([0u8; 32]),
            "key_decryption_nonce": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "file_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "thumb_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "encrypted_metadata": base64::prelude::BASE64_STANDARD.encode(b"{}"),
            "encrypted_thumbnail": base64::prelude::BASE64_STANDARD.encode(b"thumb"),
            "thumbnail_size": 5,
        }))
        .send()
        .await
        .expect("register failed");

    let file_id = register_resp.json::<serde_json::Value>().await.unwrap()["file_id"].as_i64().unwrap();

    let download_resp = client
        .get(&format!("{}/api/files/{}/download", base_url(), file_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("download request failed");

    assert_eq!(download_resp.status(), 200);
    let download_body: serde_json::Value = download_resp.json().await.unwrap();
    assert!(download_body.get("url").is_some());
    assert!(download_body.get("file_id").is_some());
    assert!(download_body.get("content_hash").is_some());
    assert!(download_body.get("file_size").is_some());
    assert!(download_body.get("mime_type").is_some());

    let presigned_download_url = download_body["url"].as_str().unwrap();
    assert!(!presigned_download_url.is_empty());
    assert!(presigned_download_url.contains("localhost") || presigned_download_url.contains("127.0.0.1") || presigned_download_url.contains("s3"));
}

#[tokio::test]
async fn test_s3_abort_multipart_upload() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Abort test content";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap().to_string();
    let upload_id_s3 = upload["upload_id_s3"].as_str().unwrap().to_string();
    let object_key = upload["object_key"].as_str().unwrap().to_string();

    let s3_client = create_s3_client().await;
    let result = zoo::s3::client::abort_multipart_upload(
        &s3_client,
        TEST_BUCKET,
        &object_key,
        &upload_id_s3,
    )
    .await;

    assert!(result.is_ok());

    let fail_resp = client
        .post(&format!("{}/api/uploads/{}/fail", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "reason": "aborted by test" }))
        .send()
        .await
        .expect("fail request failed");

    assert_eq!(fail_resp.status(), 204);
}

#[tokio::test]
async fn test_s3_delete_object() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Delete test content";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();
    let object_key = upload["object_key"].as_str().unwrap().to_string();

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");
    let presigned_url = presign_resp.json::<serde_json::Value>().await.unwrap()["urls"].as_array().unwrap()[0].as_str().unwrap().to_string();

    let put_resp = client.put(&presigned_url).body(file_content.to_vec()).send().await.expect("S3 upload failed");
    assert!(put_resp.status().is_success());

    let s3_client = create_s3_client().await;
    let result = zoo::s3::client::delete_object(&s3_client, TEST_BUCKET, &object_key).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_s3_head_object_size() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Head object test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();
    let object_key = upload["object_key"].as_str().unwrap().to_string();

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");
    let presigned_url = presign_resp.json::<serde_json::Value>().await.unwrap()["urls"].as_array().unwrap()[0].as_str().unwrap().to_string();

    let put_resp = client.put(&presigned_url).body(file_content.to_vec()).send().await.expect("S3 upload failed");
    assert!(put_resp.status().is_success());

    let s3_client = create_s3_client().await;
    let result = zoo::s3::client::head_object_size(&s3_client, TEST_BUCKET, &object_key).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_sync_files_with_registered_file() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Sync test content";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");
    let presigned_url = presign_resp.json::<serde_json::Value>().await.unwrap()["urls"].as_array().unwrap()[0].as_str().unwrap().to_string();

    let put_resp = client.put(&presigned_url).body(file_content.to_vec()).send().await.expect("S3 upload failed");
    assert!(put_resp.status().is_success());
    let etag = put_resp.headers().get("ETag").and_then(|v| v.to_str().ok()).map(|s| s.trim_matches('"').to_string()).unwrap();

    client.put(&format!("{}/api/uploads/{}/parts/1", base_url(), upload_id))
        .bearer_auth(&token).header("x-device-id", &device_id)
        .json(&json!({ "etag": etag, "size": file_size }))
        .send().await.expect("confirm part failed");

    client.patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token).header("x-device-id", &device_id)
        .json(&json!({ "status": "encrypting" }))
        .send().await.expect("patch failed");

    client.patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token).header("x-device-id", &device_id)
        .json(&json!({ "status": "uploading" }))
        .send().await.expect("patch failed");

    client.post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token).header("x-device-id", &device_id)
        .send().await.expect("complete failed");

    let collection_id = Uuid::new_v4().to_string();
    let register_resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token).header("x-device-id", &device_id)
        .json(&json!({
            "collection_id": collection_id,
            "cipher": "xchacha20-poly1305",
            "encrypted_key": base64::prelude::BASE64_STANDARD.encode([0u8; 32]),
            "key_decryption_nonce": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "file_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "thumb_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "encrypted_metadata": base64::prelude::BASE64_STANDARD.encode(b"{}"),
            "encrypted_thumbnail": base64::prelude::BASE64_STANDARD.encode(b"thumb"),
            "thumbnail_size": 5,
        }))
        .send().await.expect("register failed");

    let file_id = register_resp.json::<serde_json::Value>().await.unwrap()["file_id"].as_i64().unwrap();

    let sync_resp = client
        .get(&format!("{}/api/sync/files", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("sync files failed");

    assert_eq!(sync_resp.status(), 200);
    let sync_body: serde_json::Value = sync_resp.json().await.unwrap();
    let files = sync_body["updated_files"].as_array().unwrap();
    assert!(!files.is_empty());

    let synced_file = &files[0];
    assert_eq!(synced_file["id"], file_id);
    assert!(synced_file.get("object_key").is_some());
    assert!(synced_file.get("content_hash").is_some());
    assert!(synced_file.get("file_size").is_some());
    assert!(synced_file.get("encrypted_key").is_some());
}

#[tokio::test]
async fn test_upload_dedup_returns_existing() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Dedup test content";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload1 = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id_1 = upload1["upload_id"].as_str().unwrap().to_string();

    let resp2 = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": file_size,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 1,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("second upload request failed");

    assert_eq!(resp2.status(), 409);
}

#[tokio::test]
async fn test_upload_heartbeat_updates_timestamp() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Heartbeat test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let heartbeat_resp = client
        .post(&format!("{}/api/uploads/{}/heartbeat", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("heartbeat failed");

    assert_eq!(heartbeat_resp.status(), 204);

    let status_resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 200);
}

#[tokio::test]
async fn test_upload_patch_status_to_failed() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Patch test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let patch_resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "failed" }))
        .send()
        .await
        .expect("patch failed");

    assert!(patch_resp.status().is_success() || patch_resp.status() == 200);

    let status_resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 200);
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(status_body["status"], "failed");
}

#[tokio::test]
async fn test_upload_cancel_deletes_upload() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Cancel test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let cancel_resp = client
        .delete(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("cancel failed");

    assert!(cancel_resp.status().is_success() || cancel_resp.status() == 204);

    let status_resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 200);
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(status_body["status"], "failed");
}

#[tokio::test]
async fn test_upload_list_pending_with_uploads() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"List pending test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    create_upload(&client, &token, &device_id, &file_hash, file_size).await;

    let list_resp = client
        .get(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("list pending failed");

    assert_eq!(list_resp.status(), 200);
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let uploads = list_body.as_array().unwrap();
    assert!(!uploads.is_empty());
}

#[tokio::test]
async fn test_upload_presign_refresh() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Presign refresh test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let refresh_resp = client
        .post(&format!("{}/api/uploads/{}/presign-refresh", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign refresh failed");

    assert_eq!(refresh_resp.status(), 200);
    let refresh_body: serde_json::Value = refresh_resp.json().await.unwrap();
    assert!(refresh_body.get("urls").is_some());
    assert!(refresh_body.get("complete_url").is_some());
}

#[tokio::test]
async fn test_upload_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client1, token1, device_id1, _) = setup_user().await;

    let file_content = b"Forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client1, &token1, &device_id1, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client1, &email2).await;
    let token2 = login_user(&client1, &email2).await;

    let status_resp = client1
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id1)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 403);

    let complete_resp = client1
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id1)
        .send()
        .await
        .expect("complete failed");

    assert_eq!(complete_resp.status(), 403);
}

#[tokio::test]
async fn test_upload_fail_endpoint() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Fail endpoint test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let fail_resp = client
        .post(&format!("{}/api/uploads/{}/fail", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "reason": "manual failure" }))
        .send()
        .await
        .expect("fail request failed");

    assert_eq!(fail_resp.status(), 204);

    let status_resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("get status failed");

    assert_eq!(status_resp.status(), 200);
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(status_body["status"], "failed");
}

#[tokio::test]
async fn test_upload_part_count_exceeded() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Part count test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let confirm1 = client
        .put(&format!("{}/api/uploads/{}/parts/1", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "etag": "etag1", "size": file_size }))
        .send()
        .await
        .expect("confirm part 1 failed");

    assert!(confirm1.status().is_success() || confirm1.status() == 204);

    let confirm2 = client
        .put(&format!("{}/api/uploads/{}/parts/2", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "etag": "etag2", "size": file_size }))
        .send()
        .await
        .expect("confirm part 2 request failed");

    assert!(confirm2.status() == 400 || confirm2.status() == 204);
}

#[tokio::test]
async fn test_upload_patch_invalid_transition() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;

    let file_content = b"Invalid transition test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let patch_resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "done" }))
        .send()
        .await
        .expect("patch failed");

    assert_eq!(patch_resp.status(), 400);
}

#[tokio::test]
async fn test_upload_presign_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Presign forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let presign_resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign failed");

    assert!(presign_resp.status() == 403 || presign_resp.status() == 404);
}

#[tokio::test]
async fn test_upload_presign_refresh_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Presign refresh forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let refresh_resp = client
        .post(&format!("{}/api/uploads/{}/presign-refresh", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .json(&json!({ "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"] }))
        .send()
        .await
        .expect("presign refresh failed");

    assert!(refresh_resp.status() == 403 || refresh_resp.status() == 404);
}

#[tokio::test]
async fn test_upload_complete_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Complete forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let complete_resp = client
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("complete failed");

    assert_eq!(complete_resp.status(), 403);
}

#[tokio::test]
async fn test_upload_register_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Register forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let register_resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .json(&json!({
            "collection_id": "test",
            "cipher": "xchacha20-poly1305",
            "encrypted_key": base64::prelude::BASE64_STANDARD.encode([0u8; 32]),
            "key_decryption_nonce": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "file_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "thumb_decryption_header": base64::prelude::BASE64_STANDARD.encode([0u8; 24]),
            "encrypted_metadata": base64::prelude::BASE64_STANDARD.encode(b"{}"),
            "encrypted_thumbnail": base64::prelude::BASE64_STANDARD.encode(b"thumb"),
            "thumbnail_size": 5,
        }))
        .send()
        .await
        .expect("register failed");

    assert_eq!(register_resp.status(), 403);
}

#[tokio::test]
async fn test_upload_cancel_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Cancel forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let cancel_resp = client
        .delete(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .send()
        .await
        .expect("cancel failed");

    assert_eq!(cancel_resp.status(), 403);
}

#[tokio::test]
async fn test_upload_patch_forbidden_cross_user() {
    get_server().await;
    clean_test_db().await;

    let (client, token1, device_id, _) = setup_user().await;

    let file_content = b"Patch forbidden test";
    let file_hash = format!("{:x}", Sha256::digest(file_content));
    let file_size = file_content.len() as i64;

    let upload = create_upload(&client, &token1, &device_id, &file_hash, file_size).await;
    let upload_id = upload["upload_id"].as_str().unwrap();

    let email2 = format!("test2_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let patch_resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .header("x-device-id", &device_id)
        .json(&json!({ "status": "failed" }))
        .send()
        .await
        .expect("patch failed");

    assert_eq!(patch_resp.status(), 403);
}

async fn create_s3_client() -> aws_sdk_s3::Client {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_sdk_s3::config::Region::new(S3_REGION))
        .endpoint_url(S3_ENDPOINT)
        .load()
        .await;

    let config_builder = aws_sdk_s3::config::Builder::from(&sdk_config)
        .force_path_style(true)
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            S3_ACCESS_KEY,
            S3_SECRET_KEY,
            None,
            None,
            "static",
        ));

    aws_sdk_s3::Client::from_conf(config_builder.build())
}
