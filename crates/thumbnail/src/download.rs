use crate::decrypt::decrypt_thumbnail;
use crate::error::ThumbnailError;
use types::crypto::{Header24, Key256};

pub async fn download_thumbnail(
    fetch_encrypted: impl std::future::Future<Output = Result<Vec<u8>, String>> + Send,
    thumb_header: &Header24,
    file_key: &Key256,
) -> Result<Vec<u8>, ThumbnailError> {
    let encrypted = fetch_encrypted
        .await
        .map_err(ThumbnailError::DownloadError)?;

    let decrypted = decrypt_thumbnail(thumb_header, &encrypted, file_key)?;

    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;
    use crypto::stream_encrypt;

    #[tokio::test]
    async fn test_download_thumbnail_success() {
        let key = generate_key();
        let plaintext = b"downloaded thumbnail";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);

        let fetcher = async move { Ok(ciphertext) };
        let result = download_thumbnail(fetcher, &header, &key).await.unwrap();
        assert_eq!(result, plaintext);
    }

    #[tokio::test]
    async fn test_download_thumbnail_fetch_error() {
        let key = generate_key();
        let header = Header24::new([0u8; 24]);

        let fetcher = async move { Err("network error".to_string()) };
        let result = download_thumbnail(fetcher, &header, &key).await;
        assert!(matches!(result, Err(ThumbnailError::DownloadError(_))));
    }

    #[tokio::test]
    async fn test_download_thumbnail_decrypt_error() {
        let key = generate_key();
        let wrong_key = generate_key();
        let plaintext = b"thumbnail data";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);

        let fetcher = async move { Ok(ciphertext) };
        let result = download_thumbnail(fetcher, &header, &wrong_key).await;
        assert!(matches!(result, Err(ThumbnailError::Crypto(_))));
    }
}
