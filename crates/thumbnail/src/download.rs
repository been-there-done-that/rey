use types::crypto::{Header24, Key256};
use crate::decrypt::decrypt_thumbnail;
use crate::error::ThumbnailError;

pub async fn download_thumbnail(
    fetch_encrypted: impl std::future::Future<Output = Result<Vec<u8>, String>> + Send,
    thumb_header: &Header24,
    file_key: &Key256,
) -> Result<Vec<u8>, ThumbnailError> {
    let encrypted = fetch_encrypted
        .await
        .map_err(|e| ThumbnailError::DownloadError(e))?;

    let decrypted = decrypt_thumbnail(thumb_header, &encrypted, file_key)?;

    Ok(decrypted)
}
