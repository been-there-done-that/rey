use crate::structs::FileMetadata;
use base64::{engine::general_purpose::STANDARD, Engine};
use types::crypto::Key256;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MagicMetadata {
    pub encrypted_taken_at: Option<String>,
    pub encrypted_file_size: Option<String>,
    pub encrypted_mime_type: Option<String>,
    pub encrypted_collection_id: Option<String>,
    pub content_hash: String,
}

fn encrypt_field(value: &str, file_key: &Key256) -> String {
    let (header, ciphertext) = crypto::stream_encrypt(value.as_bytes(), file_key);
    let header_b64 = STANDARD.encode(header.as_bytes());
    let ct_b64 = STANDARD.encode(&ciphertext);
    format!("{}:{}", header_b64, ct_b64)
}

pub fn derive_magic_metadata(metadata: &FileMetadata, file_key: &Key256) -> MagicMetadata {
    let taken_at = metadata.taken_at.map(|ts| encrypt_field(&ts.to_string(), file_key));
    let file_size = metadata
        .title
        .as_ref()
        .map(|_| encrypt_field("size_placeholder", file_key));
    let mime_type = metadata
        .title
        .as_ref()
        .map(|_| encrypt_field("mime_placeholder", file_key));
    let collection_id = metadata
        .title
        .as_ref()
        .map(|_| encrypt_field("col_placeholder", file_key));

    MagicMetadata {
        encrypted_taken_at: taken_at,
        encrypted_file_size: file_size,
        encrypted_mime_type: mime_type,
        encrypted_collection_id: collection_id,
        content_hash: "hash_placeholder".to_string(),
    }
}

pub fn serialize_magic_metadata(magic: &MagicMetadata) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(magic)
}

pub fn deserialize_magic_metadata(bytes: &[u8]) -> Result<MagicMetadata, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;

    #[test]
    fn test_derive_magic_metadata_with_taken_at() {
        let key = generate_key();
        let metadata = FileMetadata {
            taken_at: Some(1700000000000),
            title: Some("photo.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };

        let magic = derive_magic_metadata(&metadata, &key);

        assert!(magic.encrypted_taken_at.is_some());
        assert!(magic.encrypted_taken_at.as_ref().unwrap().contains(':'));
    }

    #[test]
    fn test_derive_magic_metadata_without_taken_at() {
        let key = generate_key();
        let metadata = FileMetadata {
            taken_at: None,
            title: None,
            description: None,
            latitude: None,
            longitude: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };

        let magic = derive_magic_metadata(&metadata, &key);

        assert!(magic.encrypted_taken_at.is_none());
    }

    #[test]
    fn test_encrypt_field_produces_different_output() {
        let key = generate_key();
        let field1 = encrypt_field("test_value", &key);
        let field2 = encrypt_field("test_value", &key);

        assert_ne!(field1, field2);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let magic = MagicMetadata {
            encrypted_taken_at: Some("header:ciphertext".to_string()),
            encrypted_file_size: None,
            encrypted_mime_type: Some("header2:ct2".to_string()),
            encrypted_collection_id: None,
            content_hash: "abc123".to_string(),
        };

        let bytes = serialize_magic_metadata(&magic).unwrap();
        let decoded = deserialize_magic_metadata(&bytes).unwrap();

        assert_eq!(decoded.encrypted_taken_at, magic.encrypted_taken_at);
        assert_eq!(decoded.content_hash, magic.content_hash);
    }

    #[test]
    fn test_magic_metadata_json_is_valid() {
        let key = generate_key();
        let metadata = FileMetadata {
            taken_at: Some(1700000000000),
            title: Some("test.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };

        let magic = derive_magic_metadata(&metadata, &key);
        let bytes = serialize_magic_metadata(&magic).unwrap();
        let json_str = String::from_utf8(bytes.clone()).unwrap();

        assert!(json_str.contains("encrypted_taken_at"));
        assert!(json_str.contains("content_hash"));
    }
}
