use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::process::{Child, Command};
use std::time::Duration;
use uuid::Uuid;
use zoo_client::ZooClient;
use base64::Engine;

const TEST_DB: &str = "zoo_client_test";
const TEST_BUCKET: &str = "test-bucket";
const S3_ENDPOINT: &str = "http://localhost:4566";
const S3_REGION: &str = "us-east-1";
const S3_ACCESS_KEY: &str = "test";
const S3_SECRET_KEY: &str = "test";

fn test_db_url() -> String {
    format!("postgres://postgres:postgres@localhost/{}", TEST_DB)
}

fn ensure_test_db() {
    let output = Command::new("psql")
        .args([
            "-h", "localhost",
            "-U", "postgres",
            "-c",
            &format!("SELECT 1 FROM pg_database WHERE datname = '{}'", TEST_DB),
        ])
        .env("PGPASSWORD", "postgres")
        .output()
        .expect("failed to check database");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("(1 row)") {
        Command::new("psql")
            .args([
                "-h", "localhost",
                "-U", "postgres",
                "-c",
                &format!("CREATE DATABASE {}", TEST_DB),
            ])
            .env("PGPASSWORD", "postgres")
            .output()
            .expect("failed to create test database");
    }

    let migrations_dir = env::current_dir()
        .unwrap()
        .join("crates/zoo/migrations");

    for entry in std::fs::read_dir(&migrations_dir).expect("migrations dir not found") {
        let entry = entry.expect("failed to read entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            Command::new("psql")
                .args([
                    "-h", "localhost",
                    "-U", "postgres",
                    "-d", TEST_DB,
                    "-f",
                    path.to_str().unwrap(),
                ])
                .env("PGPASSWORD", "postgres")
                .output()
                .expect("failed to run migration");
        }
    }
}

fn clean_test_db() {
    let tables = [
        "shares", "files", "upload_parts", "uploads", "devices", "sessions", "users",
    ];
    for table in &tables {
        Command::new("psql")
            .args([
                "-h", "localhost",
                "-U", "postgres",
                "-d", TEST_DB,
                "-c",
                &format!("TRUNCATE {} CASCADE", table),
            ])
            .env("PGPASSWORD", "postgres")
            .output()
            .ok();
    }
}

struct TestServer {
    port: u16,
    child: Child,
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

        Self { port, child }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn wait_for_ready(&self, max_retries: usize) {
        let client = reqwest::blocking::Client::new();
        for i in 0..max_retries {
            if client.get(&self.base_url()).send().is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(500 * (i as u64 + 1)));
        }
        panic!("server did not start within timeout");
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn register_and_login(client: &Client, base_url: &str, email: &str) -> String {
    let verify_key_hash = {
        let hash = format!("{:x}", Sha256::digest(b"test_verify_key"));
        bcrypt::hash(&hash, bcrypt::DEFAULT_COST).unwrap()
    };

    let kek_salt = base64::prelude::BASE64_STANDARD.encode([0u8; 16]);
    let encrypted_master_key = base64::prelude::BASE64_STANDARD.encode([0u8; 32]);
    let key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let public_key = base64::prelude::BASE64_STANDARD.encode([0u8; 32]);
    let encrypted_secret_key = base64::prelude::BASE64_STANDARD.encode([0u8; 48]);
    let secret_key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);
    let encrypted_recovery_key = base64::prelude::BASE64_STANDARD.encode([0u8; 48]);
    let recovery_key_nonce = base64::prelude::BASE64_STANDARD.encode([0u8; 24]);

    let _ = client
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

    let resp = client
        .post(&format!("{}/api/auth/login", base_url))
        .json(&json!({
            "email": email,
            "verify_key_hash": verify_key_hash,
        }))
        .send()
        .await
        .expect("login failed");

    let body: serde_json::Value = resp.json().await.expect("login response not json");
    body["session_token"]
        .as_str()
        .expect("missing session_token")
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
    ensure_test_db();
    clean_test_db();

    let port = 3030;
    let server = TestServer::new(port);
    server.wait_for_ready(20);

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    let uploads = client.pending_uploads().await.expect("pending_uploads failed");
    assert!(uploads.is_empty());
}

#[tokio::test]
async fn test_zoo_client_cancel_upload() {
    ensure_test_db();
    clean_test_db();

    let port = 3031;
    let server = TestServer::new(port);
    server.wait_for_ready(20);

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
    ensure_test_db();
    clean_test_db();

    let port = 3032;
    let server = TestServer::new(port);
    server.wait_for_ready(20);

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
    ensure_test_db();
    clean_test_db();

    let port = 3033;
    let server = TestServer::new(port);
    server.wait_for_ready(20);

    let base_url = server.base_url();
    let email = format!("test_{}@example.com", Uuid::new_v4());
    let http_client = Client::new();
    let token = register_and_login(&http_client, &base_url, &email).await;

    let client = ZooClient::new(base_url);
    client.set_session_token(token);

    let result = client.get_thumbnail(99999).await;
    assert!(result.is_err());
}
