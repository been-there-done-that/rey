use types::crypto::{Header24, Key256};
use crate::error::ThumbnailError;

pub fn decrypt_thumbnail(
    header: &Header24,
    ciphertext: &[u8],
    file_key: &Key256,
) -> Result<Vec<u8>, ThumbnailError> {
    let plaintext = crypto::stream_decrypt(header, ciphertext, file_key)
        .map_err(ThumbnailError::Crypto)?;
    Ok(plaintext)
}
