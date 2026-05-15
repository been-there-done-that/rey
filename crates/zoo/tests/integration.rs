use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::process::{Child, Command};
use std::time::Duration;
use uuid::Uuid;
use base64::Engine;
use tokio::sync::OnceCell;

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

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
    )
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
        "shares", "files", "upload_parts", "uploads", "devices", "sessions", "users",
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
    _child: Child,
}

impl TestServer {
    fn new(port: u16) -> Self {
        let child = Command::new("cargo")
            .args([
                "run",
                "-p", "zoo",
                "--bin", "zoo-server",
            ])
            .env("DATABASE_URL", &test_db_url())
            .env("LISTEN_ADDR", &format!("127.0.0.1:{}", port))
            .env("S3_ENDPOINT", S3_ENDPOINT)
            .env("S3_REGION", S3_REGION)
            .env("S3_BUCKET", TEST_BUCKET)
            .env("S3_ACCESS_KEY", S3_ACCESS_KEY)
            .env("S3_SECRET_KEY", S3_SECRET_KEY)
            .env("DOWNLOAD_MODE", "redirect")
            .env("SESSION_TTL_DAYS", "30")
            .env("STALL_TIMEOUT_SECONDS", "90")
            .env("PRESIGNED_TTL_HOURS", "24")
            .env("GC_INTERVAL_SECONDS", "300")
            .env("MAX_FILE_SIZE", "10737418240")
            .env("DEFAULT_PART_SIZE", "20971520")
            .spawn()
            .expect("failed to start zoo server");

        Self { port, _child: child }
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
        let _ = self._child.kill();
        let _ = self._child.wait();
    }
}

static SERVER: OnceCell<TestServer> = OnceCell::const_new();

async fn get_server() -> &'static TestServer {
    SERVER.get_or_init(|| async {
        ensure_test_db().await;
        let server = TestServer::new(TEST_PORT);
        server.wait_for_ready(30).await;
        server
    }).await
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

    assert!(resp.status() == 201 || resp.status() == 400, "register failed: {:?}", resp.text().await.unwrap());
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

async fn create_upload(client: &Client, token: &str, device_id: &str, file_hash: &str, file_size: i64) -> serde_json::Value {
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
        .post(&format!("{}/api/devices/{}/heartbeat", base_url(), device_id))
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
async fn test_device_not_found() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;

    let resp = client
        .delete(&format!("{}/api/devices/{}", base_url(), Uuid::new_v4()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("deregister device failed");

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_upload_create() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    assert!(body.get("upload_id").is_some());
    assert_eq!(body["status"], "pending");
}

#[tokio::test]
async fn test_upload_get_status() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get upload failed");

    assert_eq!(resp.status(), 200);
    let state: serde_json::Value = resp.json().await.expect("upload state not json");
    assert_eq!(state["status"], "pending");
}

#[tokio::test]
async fn test_upload_heartbeat() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .post(&format!("{}/api/uploads/{}/heartbeat", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("heartbeat failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_upload_complete() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    // Transition: pending -> encrypting
    let resp1 = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch to encrypting failed");
    assert_eq!(resp1.status(), 200, "patch to encrypting failed: {:?}", resp1.text().await);

    // Transition: encrypting -> uploading
    let resp2 = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "uploading" }))
        .send()
        .await
        .expect("patch to uploading failed");
    assert_eq!(resp2.status(), 200, "patch to uploading failed: {:?}", resp2.text().await);

    let resp = client
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("complete upload failed");

    assert_eq!(resp.status(), 200);
    let state: serde_json::Value = resp.json().await.expect("upload state not json");
    assert_eq!(state["status"], "s3_completed");
}

#[tokio::test]
async fn test_upload_fail() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .post(&format!("{}/api/uploads/{}/fail", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "reason": "test failure" }))
        .send()
        .await
        .expect("fail upload failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_upload_dedup() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let resp1 = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
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
        .expect("create upload 1 failed");

    assert_eq!(resp1.status(), 201);

    let resp2 = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
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
        .expect("create upload 2 failed");

    assert_eq!(resp2.status(), 409);
}

#[tokio::test]
async fn test_upload_cancel() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .delete(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("cancel upload failed");

    assert_eq!(resp.status(), 200);

    let resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get upload failed");

    let state: serde_json::Value = resp.json().await.expect("upload state not json");
    assert_eq!(state["status"], "failed");
}

#[tokio::test]
async fn test_upload_patch_status() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch upload failed");

    assert_eq!(resp.status(), 200);

    let resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get upload failed");

    let state: serde_json::Value = resp.json().await.expect("upload state not json");
    assert_eq!(state["status"], "encrypting");
}

#[tokio::test]
async fn test_upload_patch_invalid_transition() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "done" }))
        .send()
        .await
        .expect("patch upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_presign() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("presign failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("presign response not json");
    assert!(body.get("urls").is_some());
    assert!(body.get("complete_url").is_some());
}

#[tokio::test]
async fn test_upload_presign_refresh() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .post(&format!("{}/api/uploads/{}/presign-refresh", base_url(), upload_id))
        .bearer_auth(&token)
        .send()
        .await
        .expect("presign-refresh failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("presign-refresh response not json");
    assert!(body.get("urls").is_some());
    assert!(body.get("complete_url").is_some());
}

#[tokio::test]
async fn test_upload_register_file() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    // Transition through states: pending -> encrypting -> uploading -> s3_completed
    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch to encrypting failed");

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "uploading" }))
        .send()
        .await
        .expect("patch to uploading failed");

    client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "status": "s3_completed" }))
        .send()
        .await
        .expect("patch to s3_completed failed");

    let resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({
            "collection_id": "test-collection",
            "encrypted_key": "enc-key",
            "key_decryption_nonce": "nonce",
            "file_decryption_header": "header",
            "encrypted_metadata": "meta",
        }))
        .send()
        .await
        .expect("register failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("register response not json");
    assert!(body.get("file_id").is_some());
}

#[tokio::test]
async fn test_upload_list_pending() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
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

    assert_eq!(resp.status(), 201);

    let resp = client
        .get(&format!("{}/api/uploads?status=pending", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("list uploads failed");

    assert_eq!(resp.status(), 200);
    let uploads: Vec<serde_json::Value> = resp.json().await.expect("list uploads not json");
    assert!(!uploads.is_empty());
    assert_eq!(uploads[0]["status"], "pending");
}

#[tokio::test]
async fn test_upload_confirm_part() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token, &device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let resp = client
        .put(&format!("{}/api/uploads/{}/parts/1", base_url(), upload_id))
        .bearer_auth(&token)
        .json(&json!({ "etag": "test-etag-123" }))
        .send()
        .await
        .expect("confirm part failed");

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_upload_missing_device_header() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
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

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_invalid_device_id_format() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;
    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", "not-a-uuid")
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

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_file_too_large() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_hash = format!("{:x}", Sha256::digest(b"test"));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": 11000000000i64,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 1,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("create upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_part_count_exceeded() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_hash = format!("{:x}", Sha256::digest(b"test"));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": 1024,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 10001,
            "part_md5s": [],
        }))
        .send()
        .await
        .expect("create upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_part_size_too_small() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_hash = format!("{:x}", Sha256::digest(b"test"));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": 1024,
            "mime_type": "application/octet-stream",
            "part_size": 1024,
            "part_count": 1,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("create upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_part_md5s_count_mismatch() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_hash = format!("{:x}", Sha256::digest(b"test"));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": 1024,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 2,
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("create upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_part_md5_invalid_length() {
    get_server().await;
    clean_test_db().await;

    let (client, token, device_id, _) = setup_user().await;
    let file_hash = format!("{:x}", Sha256::digest(b"test"));

    let resp = client
        .post(&format!("{}/api/uploads", base_url()))
        .bearer_auth(&token)
        .header("x-device-id", &device_id)
        .json(&json!({
            "file_hash": file_hash,
            "file_size": 1024,
            "mime_type": "application/octet-stream",
            "part_size": 5242880,
            "part_count": 1,
            "part_md5s": ["short"],
        }))
        .send()
        .await
        .expect("create upload failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_not_found() {
    get_server().await;
    clean_test_db().await;

    let (client, token, _, _) = setup_user().await;

    let resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), Uuid::new_v4()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get upload failed");

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_upload_forbidden_access() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .get(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .send()
        .await
        .expect("get upload failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_cancel_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .delete(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .send()
        .await
        .expect("cancel upload failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_patch_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .patch(&format!("{}/api/uploads/{}", base_url(), upload_id))
        .bearer_auth(&token2)
        .json(&json!({ "status": "encrypting" }))
        .send()
        .await
        .expect("patch upload failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_presign_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .post(&format!("{}/api/uploads/{}/presign", base_url(), upload_id))
        .bearer_auth(&token2)
        .json(&json!({
            "part_md5s": ["d41d8cd98f00b204e9800998ecf8427e"],
        }))
        .send()
        .await
        .expect("presign failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_presign_refresh_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .post(&format!("{}/api/uploads/{}/presign-refresh", base_url(), upload_id))
        .bearer_auth(&token2)
        .send()
        .await
        .expect("presign-refresh failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_register_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .post(&format!("{}/api/uploads/{}/register", base_url(), upload_id))
        .bearer_auth(&token2)
        .json(&json!({
            "collection_id": "test-collection",
            "encrypted_key": "enc-key",
            "key_decryption_nonce": "nonce",
            "file_decryption_header": "header",
            "encrypted_metadata": "meta",
        }))
        .send()
        .await
        .expect("register failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_complete_forbidden() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device1 = register_device(&client, &token1).await;
    let device_id = device1["device_id"].as_str().unwrap();

    let file_data = vec![0u8; 1024];
    let file_hash = format!("{:x}", Sha256::digest(&file_data));

    let body = create_upload(&client, &token1, device_id, &file_hash, file_data.len() as i64).await;
    let upload_id = body["upload_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .post(&format!("{}/api/uploads/{}/complete", base_url(), upload_id))
        .bearer_auth(&token2)
        .send()
        .await
        .expect("complete upload failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_device_forbidden_access() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email1 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email1).await;
    let token1 = login_user(&client, &email1).await;
    let device = register_device(&client, &token1).await;
    let device_id = device["device_id"].as_str().unwrap();

    let email2 = format!("test_{}@example.com", Uuid::new_v4());
    register_test_user(&client, &email2).await;
    let token2 = login_user(&client, &email2).await;

    let resp = client
        .delete(&format!("{}/api/devices/{}", base_url(), device_id))
        .bearer_auth(&token2)
        .send()
        .await
        .expect("deregister device failed");

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_sync_files() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;

    let resp = client
        .get(&format!("{}/api/sync/files?since=0&limit=100", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("sync files failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("sync response not json");
    assert!(body.get("updated_files").is_some());
    assert!(body.get("deleted_file_ids").is_some());
    assert!(body.get("has_more").is_some());
    assert!(body.get("latest_updated_at").is_some());
}

#[tokio::test]
async fn test_sync_files_with_since() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;

    let resp = client
        .get(&format!("{}/api/sync/files?since=1700000000000&limit=50", base_url()))
        .bearer_auth(&token)
        .send()
        .await
        .expect("sync files failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("sync response not json");
    assert!(body.get("updated_files").is_some());
    assert!(body.get("has_more").is_some());
}

#[tokio::test]
async fn test_sse_events() {
    get_server().await;
    clean_test_db().await;

    let client = Client::new();
    let email = format!("test_{}@example.com", Uuid::new_v4());

    register_test_user(&client, &email).await;
    let token = login_user(&client, &email).await;
    register_device(&client, &token).await;

    let resp = client
        .get(&format!("{}/api/events", base_url()))
        .bearer_auth(&token)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("sse request failed");

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("text/event-stream"));
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

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_file_archive_not_found() {
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
}
