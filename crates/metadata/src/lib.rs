pub mod decrypt;
pub mod encrypt;
pub mod error;
pub mod structs;

pub use decrypt::decrypt_metadata;
pub use encrypt::encrypt_metadata;
pub use error::MetadataError;
pub use structs::FileMetadata;

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;
    use types::crypto::Key256;

    fn make_key() -> Key256 {
        generate_key()
    }

    #[test]
    fn missing_optional_fields_serialize_as_null() {
        let metadata = FileMetadata {
            title: None,
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("null") || json.contains("[]"));
    }

    #[test]
    fn json_roundtrip_preserves_all_fields() {
        let metadata = FileMetadata {
            title: Some("test.jpg".into()),
            description: Some("A test file".into()),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            taken_at: Some(1700000000000),
            device_make: Some("Apple".into()),
            device_model: Some("iPhone 15".into()),
            tags: vec!["vacation".into(), "summer".into()],
        };
        let json = serde_json::to_vec(&metadata).unwrap();
        let roundtripped: FileMetadata = serde_json::from_slice(&json).unwrap();
        assert_eq!(roundtripped.title, metadata.title);
        assert_eq!(roundtripped.tags, metadata.tags);
    }

    fn make_empty_metadata() -> FileMetadata {
        FileMetadata {
            title: None,
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        }
    }

    #[test]
    fn wrong_key_returns_mac_mismatch() {
        let metadata = FileMetadata {
            latitude: Some(40.7127753),
            longitude: Some(-74.0059728),
            title: None,
            description: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };
        let key1 = make_key();
        let key2 = make_key();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key1).unwrap();
        let result = decrypt_metadata(&header, &ciphertext, &key2);
        assert!(matches!(result, Err(MetadataError::Crypto(_))));
    }

    #[test]
    fn tags_vec_roundtrips_correctly() {
        let metadata = FileMetadata {
            tags: vec!["a".into(), "b".into(), "c".into()],
            title: None,
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
        };
        let key = make_key();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted.tags, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_metadata_roundtrips_correctly() {
        let metadata = FileMetadata {
            title: None,
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };
        let key = make_key();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted.title, metadata.title);
        assert_eq!(decrypted.tags, metadata.tags);
    }

    #[test]
    fn tampered_ciphertext_returns_mac_mismatch() {
        let metadata = make_empty_metadata();
        let key = make_key();
        let (header, mut ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = decrypt_metadata(&header, &ciphertext, &key);
        assert!(matches!(result, Err(MetadataError::Crypto(crypto::error::CryptoError::MacMismatch))));
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_time() {
        let metadata = make_empty_metadata();
        let key = make_key();
        let (_, ct1) = encrypt_metadata(&metadata, &key).unwrap();
        let (_, ct2) = encrypt_metadata(&metadata, &key).unwrap();
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn full_metadata_roundtrip() {
        let metadata = FileMetadata {
            title: Some("photo.jpg".into()),
            description: Some("Sunset at the beach".into()),
            latitude: Some(34.0522),
            longitude: Some(-118.2437),
            taken_at: Some(1700000000000),
            device_make: Some("Apple".into()),
            device_model: Some("iPhone 15 Pro".into()),
            tags: vec!["sunset".into(), "beach".into()],
        };
        let key = make_key();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted.title, metadata.title);
        assert_eq!(decrypted.description, metadata.description);
        assert_eq!(decrypted.latitude, metadata.latitude);
        assert_eq!(decrypted.longitude, metadata.longitude);
        assert_eq!(decrypted.taken_at, metadata.taken_at);
        assert_eq!(decrypted.device_make, metadata.device_make);
        assert_eq!(decrypted.device_model, metadata.device_model);
        assert_eq!(decrypted.tags, metadata.tags);
    }
}
