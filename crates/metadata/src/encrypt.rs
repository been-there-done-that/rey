use crate::error::MetadataError;
use types::crypto::{Header24, Key256};
use types::file::FileMetadata;

pub fn encrypt_metadata(
    metadata: &FileMetadata,
    file_key: &Key256,
) -> Result<(Header24, Vec<u8>), MetadataError> {
    let json_bytes = serde_json::to_vec(metadata)?;
    let (header, ciphertext) = crypto::stream_encrypt(&json_bytes, file_key);
    Ok((header, ciphertext))
}
