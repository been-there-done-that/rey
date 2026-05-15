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
                tracing::warn!(
                    "decryption failed for file {}: {}",
                    record.id,
                    e
                );
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
        .map_err(|e| SyncError::ParseError(format!("base64 decode file_decryption_header: {}", e)))?;

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

    let file_metadata: types::file::FileMetadata =
        serde_json::from_slice(&metadata_plaintext).map_err(|e| SyncError::ParseError(format!(
            "invalid metadata for file {}: {}",
            record.id, e
        )))?;

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
