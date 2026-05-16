use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub verify_key_hash: String,
    pub encrypted_master_key: String,
    pub key_nonce: String,
    pub kek_salt: String,
    pub mem_limit: i32,
    pub ops_limit: i32,
    pub public_key: String,
    pub encrypted_secret_key: String,
    pub secret_key_nonce: String,
    pub encrypted_recovery_key: String,
    pub recovery_key_nonce: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub platform: String,
    pub sse_token: String,
    pub push_token: Option<String>,
    pub stall_timeout_seconds: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Upload {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub status: String,
    pub file_hash: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub part_size: i32,
    pub part_count: i16,
    pub parts_bitmask: Option<Vec<u8>>,
    pub object_key: Option<String>,
    pub upload_id_s3: Option<String>,
    pub complete_url: Option<String>,
    pub urls_expire_at: Option<DateTime<Utc>>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub stalled_at: Option<DateTime<Utc>>,
    pub error_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub done_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct UploadPart {
    pub id: i64,
    pub upload_id: Uuid,
    pub part_number: i16,
    pub part_size: i32,
    pub part_md5: String,
    pub etag: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub uploaded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub user_id: Uuid,
    pub collection_id: String,
    pub cipher: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub encrypted_metadata: String,
    pub encrypted_thumbnail: Option<String>,
    pub thumbnail_size: Option<i32>,
    pub file_size: i64,
    pub mime_type: String,
    pub content_hash: String,
    pub object_key: String,
    pub created_at: DateTime<Utc>,
    pub updation_time: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub encrypted_name: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub encrypted_metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updation_time: DateTime<Utc>,
}
