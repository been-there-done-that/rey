use crate::error::SyncError;
use base64::Engine;
use crypto::Key256;
use types::crypto::{Header24, Nonce24};
use types::file::FileRecord;

pub fn batch_decrypt_files(
    records: &[types::file::EncryptedFileRecord],
    collection_key: &Key256,
) -> Result<Vec<FileRecord>, SyncError> {
    let mut decrypted = Vec::new();

    for record in records {
        match decrypt_single(record, collection_key) {
            Ok(file_record) => decrypted.push(file_record),
            Err(e) => {
                tracing::warn!("decryption failed for file {}: {}", record.id, e);
                continue;
            }
        }
    }

    Ok(decrypted)
}

fn decrypt_single(
    record: &types::file::EncryptedFileRecord,
    collection_key: &Key256,
) -> Result<FileRecord, SyncError> {
    let encrypted_key_bytes = base64::prelude::BASE64_STANDARD
        .decode(&record.encrypted_key)
        .map_err(|e| SyncError::ParseError(format!("base64 decode encrypted_key: {}", e)))?;

    let key_nonce_bytes = base64::prelude::BASE64_STANDARD
        .decode(&record.key_decryption_nonce)
        .map_err(|e| SyncError::ParseError(format!("base64 decode key_nonce: {}", e)))?;

    let mut nonce_arr = [0u8; 24];
    nonce_arr.copy_from_slice(&key_nonce_bytes);

    let file_key_bytes = crypto::aead::secretbox::secretbox_decrypt(
        &Nonce24::new(nonce_arr),
        &encrypted_key_bytes,
        collection_key,
    )
    .map_err(|e| SyncError::DecryptionFailed {
        file_id: record.id,
        source: e,
    })?;

    let mut file_key_arr = [0u8; 32];
    file_key_arr.copy_from_slice(&file_key_bytes);
    let file_key = Key256::new(file_key_arr);

    let metadata_bytes = base64::prelude::BASE64_STANDARD
        .decode(&record.encrypted_metadata)
        .map_err(|e| SyncError::ParseError(format!("base64 decode encrypted_metadata: {}", e)))?;

    let header_bytes = base64::prelude::BASE64_STANDARD
        .decode(&record.file_decryption_header)
        .map_err(|e| {
            SyncError::ParseError(format!("base64 decode file_decryption_header: {}", e))
        })?;

    let mut header_arr = [0u8; 24];
    header_arr.copy_from_slice(&header_bytes);

    let metadata_plaintext = crypto::aead::stream::stream_decrypt(
        &Header24::new(header_arr),
        &metadata_bytes,
        &file_key,
    )
    .map_err(|e| SyncError::DecryptionFailed {
        file_id: record.id,
        source: e,
    })?;

    let file_metadata: types::file::FileMetadata = serde_json::from_slice(&metadata_plaintext)
        .map_err(|e| {
            SyncError::ParseError(format!("invalid metadata for file {}: {}", record.id, e))
        })?;

    Ok(FileRecord {
        id: record.id,
        collection_id: record.collection_id.clone(),
        cipher: record.cipher.clone(),
        title: file_metadata.title,
        description: file_metadata.description,
        latitude: file_metadata.latitude,
        longitude: file_metadata.longitude,
        taken_at: file_metadata.taken_at,
        file_size: record.file_size,
        mime_type: record.mime_type.clone(),
        content_hash: record.content_hash.clone(),
        encrypted_key: record.encrypted_key.clone(),
        key_nonce: record.key_decryption_nonce.clone(),
        file_decryption_header: record.file_decryption_header.clone(),
        thumb_decryption_header: record.thumb_decryption_header.clone(),
        object_key: record.object_key.clone(),
        thumbnail_path: None,
        updation_time: record.updation_time,
        created_at: record.created_at,
        archived_at: record.archived_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use crypto::key::generate_key;
    use types::file::EncryptedFileRecord;

    fn make_encrypted_record(
        id: i64,
        collection_key: &Key256,
        metadata: &types::file::FileMetadata,
    ) -> EncryptedFileRecord {
        let file_key = generate_key();

        let (key_nonce, encrypted_key) =
            crypto::aead::secretbox::secretbox_encrypt(file_key.as_bytes(), collection_key);

        let (header, ciphertext) = crypto::aead::stream::stream_encrypt(
            &serde_json::to_vec(metadata).unwrap(),
            &file_key,
        );

        EncryptedFileRecord {
            id,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            encrypted_key: base64::prelude::BASE64_STANDARD.encode(&encrypted_key),
            key_decryption_nonce: base64::prelude::BASE64_STANDARD.encode(key_nonce.as_bytes()),
            file_decryption_header: base64::prelude::BASE64_STANDARD.encode(header.as_bytes()),
            thumb_decryption_header: None,
            encrypted_metadata: base64::prelude::BASE64_STANDARD.encode(&ciphertext),
            encrypted_thumbnail: None,
            thumbnail_size: None,
            file_size: 1024,
            mime_type: "image/jpeg".to_string(),
            content_hash: "hash123".to_string(),
            object_key: "obj/key".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        }
    }

    #[test]
    fn test_batch_decrypt_files_empty() {
        let key = generate_key();
        let result = batch_decrypt_files(&[], &key).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_batch_decrypt_files_single_record() {
        let collection_key = generate_key();
        let metadata = types::file::FileMetadata {
            title: Some("test.jpg".to_string()),
            description: Some("A test file".to_string()),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            taken_at: Some(1700000000000),
            device_make: Some("Apple".to_string()),
            device_model: Some("iPhone".to_string()),
            tags: vec!["test".to_string()],
        };

        let record = make_encrypted_record(1, &collection_key, &metadata);
        let result = batch_decrypt_files(&[record], &collection_key).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].title, Some("test.jpg".to_string()));
        assert_eq!(result[0].file_size, 1024);
    }

    #[test]
    fn test_batch_decrypt_files_skips_invalid_records() {
        let collection_key = generate_key();
        let metadata = types::file::FileMetadata {
            title: Some("valid.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };

        let valid_record = make_encrypted_record(1, &collection_key, &metadata);

        let invalid_record = EncryptedFileRecord {
            id: 2,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            encrypted_key: "not-valid-base64!@#".to_string(),
            key_decryption_nonce: "also-invalid".to_string(),
            file_decryption_header: "invalid".to_string(),
            thumb_decryption_header: None,
            encrypted_metadata: "bad".to_string(),
            encrypted_thumbnail: None,
            thumbnail_size: None,
            file_size: 512,
            mime_type: "image/png".to_string(),
            content_hash: "hash".to_string(),
            object_key: "obj".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };

        let result = batch_decrypt_files(&[valid_record, invalid_record], &collection_key).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[test]
    fn test_batch_decrypt_files_wrong_key_returns_empty() {
        let correct_key = generate_key();
        let wrong_key = generate_key();
        let metadata = types::file::FileMetadata {
            title: Some("secret.jpg".to_string()),
            description: None,
            latitude: None,
            longitude: None,
            taken_at: None,
            device_make: None,
            device_model: None,
            tags: vec![],
        };

        let record = make_encrypted_record(1, &correct_key, &metadata);
        let result = batch_decrypt_files(&[record], &wrong_key).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn test_decrypt_single_preserves_all_fields() {
        let collection_key = generate_key();
        let metadata = types::file::FileMetadata {
            title: Some("photo.jpg".to_string()),
            description: Some("Sunset".to_string()),
            latitude: Some(34.0522),
            longitude: Some(-118.2437),
            taken_at: Some(1700000000000),
            device_make: Some("Canon".to_string()),
            device_model: Some("EOS R5".to_string()),
            tags: vec!["sunset".to_string(), "nature".to_string()],
        };

        let record = make_encrypted_record(42, &collection_key, &metadata);
        let result = decrypt_single(&record, &collection_key).unwrap();

        assert_eq!(result.id, 42);
        assert_eq!(result.title, metadata.title);
        assert_eq!(result.description, metadata.description);
        assert_eq!(result.latitude, metadata.latitude);
        assert_eq!(result.longitude, metadata.longitude);
        assert_eq!(result.taken_at, metadata.taken_at);
        assert_eq!(result.file_size, 1024);
        assert_eq!(result.mime_type, "image/jpeg");
        assert_eq!(result.thumbnail_path, None);
    }
}
