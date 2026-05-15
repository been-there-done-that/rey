use types::crypto::{Header24, Key256};
use types::file::FileMetadata;
use crate::error::MetadataError;

pub fn decrypt_metadata(
    header: &Header24,
    ciphertext: &[u8],
    file_key: &Key256,
) -> Result<FileMetadata, MetadataError> {
    let plaintext_bytes = crypto::stream_decrypt(header, ciphertext, file_key)?;
    let metadata = serde_json::from_slice(&plaintext_bytes)?;
    Ok(metadata)
}
