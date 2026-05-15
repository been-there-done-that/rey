use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRequest {
    pub file_hash: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub part_size: i32,
    pub part_count: u16,
    pub part_md5s: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub collection_id: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub encrypted_metadata: String,
    pub encrypted_thumbnail: Option<String>,
    pub thumbnail_size: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegisterRequest {
    pub name: String,
    pub platform: String,
    pub push_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQuery {
    pub since: Option<i64>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}
