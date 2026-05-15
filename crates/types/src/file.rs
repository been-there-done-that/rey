use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,
    pub device_make: Option<String>,
    pub device_model: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFileRecord {
    pub id: i64,
    pub collection_id: String,
    #[serde(default = "default_cipher")]
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
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

fn default_cipher() -> String {
    "xchacha20-poly1305".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub collection_id: String,
    pub cipher: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub taken_at: Option<i64>,
    pub file_size: i64,
    pub mime_type: String,
    pub content_hash: String,
    pub encrypted_key: String,
    pub key_nonce: String,
    pub file_decryption_header: String,
    pub thumb_decryption_header: Option<String>,
    pub object_key: String,
    pub thumbnail_path: Option<String>,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metadata_roundtrip() {
        let fm = FileMetadata {
            title: Some("test.jpg".to_string()),
            description: Some("A test file".to_string()),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            taken_at: Some(1700000000000),
            device_make: Some("Apple".to_string()),
            device_model: Some("iPhone 15".to_string()),
            tags: vec!["vacation".to_string()],
        };
        let json = serde_json::to_string(&fm).unwrap();
        let decoded: FileMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.title, fm.title);
        assert_eq!(decoded.tags, fm.tags);
    }

    #[test]
    fn test_encrypted_file_record_roundtrip() {
        let efr = EncryptedFileRecord {
            id: 1,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            encrypted_key: "enc_key".to_string(),
            key_decryption_nonce: "nonce".to_string(),
            file_decryption_header: "header".to_string(),
            thumb_decryption_header: None,
            encrypted_metadata: "meta".to_string(),
            encrypted_thumbnail: None,
            thumbnail_size: None,
            file_size: 1024,
            mime_type: "image/jpeg".to_string(),
            content_hash: "hash123".to_string(),
            object_key: "obj/key".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&efr).unwrap();
        let decoded: EncryptedFileRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, efr.id);
        assert_eq!(decoded.file_size, efr.file_size);
    }

    #[test]
    fn test_file_record_roundtrip() {
        let fr = FileRecord {
            id: 1,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            title: Some("test.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            file_size: 2048,
            mime_type: "image/png".to_string(),
            content_hash: "hash".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            file_decryption_header: "fdh".to_string(),
            thumb_decryption_header: None,
            object_key: "ok".to_string(),
            thumbnail_path: Some("/tmp/thumb".to_string()),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&fr).unwrap();
        let decoded: FileRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.title, fr.title);
    }
}
