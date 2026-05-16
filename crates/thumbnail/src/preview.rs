use crate::cache::ThumbnailCache;
use crate::decrypt::decrypt_thumbnail;
use crate::error::ThumbnailError;
use types::crypto::{Header24, Key256};

pub struct PreviewResult {
    pub thumbnail_bytes: Option<Vec<u8>>,
    pub file_id: i64,
    pub is_placeholder: bool,
}

const PLACEHOLDER_JPEG: &[u8] = include_bytes!("../assets/placeholder.jpg");

pub async fn generate_preview(
    cache: &ThumbnailCache,
    file_id: i64,
    file_key: &Key256,
    thumb_header: Option<&Header24>,
    fetcher: impl FnOnce() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Vec<u8>, ThumbnailError>> + Send>,
        > + Send,
) -> PreviewResult {
    let Some(header) = thumb_header else {
        return PreviewResult {
            thumbnail_bytes: None,
            file_id,
            is_placeholder: true,
        };
    };

    match cache.get(file_id, file_key, header, fetcher).await {
        Ok(bytes) => PreviewResult {
            thumbnail_bytes: Some(bytes),
            file_id,
            is_placeholder: false,
        },
        Err(_) => PreviewResult {
            thumbnail_bytes: None,
            file_id,
            is_placeholder: true,
        },
    }
}

pub fn placeholder_bytes() -> &'static [u8] {
    PLACEHOLDER_JPEG
}

pub fn generate_preview_sync(
    encrypted_thumbnail: Option<&[u8]>,
    thumb_header: Option<&Header24>,
    file_key: &Key256,
) -> PreviewResult {
    let (Some(ciphertext), Some(header)) = (encrypted_thumbnail, thumb_header) else {
        return PreviewResult {
            thumbnail_bytes: None,
            file_id: 0,
            is_placeholder: true,
        };
    };

    match decrypt_thumbnail(header, ciphertext, file_key) {
        Ok(bytes) => PreviewResult {
            thumbnail_bytes: Some(bytes),
            file_id: 0,
            is_placeholder: false,
        },
        Err(_) => PreviewResult {
            thumbnail_bytes: None,
            file_id: 0,
            is_placeholder: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encrypt::encrypt_thumbnail;
    use crypto::key::generate_key;

    #[test]
    fn test_placeholder_bytes_not_empty() {
        let placeholder = placeholder_bytes();
        assert!(!placeholder.is_empty());
    }

    #[test]
    fn test_generate_preview_sync_with_valid_thumbnail() {
        let key = generate_key();
        let plaintext = b"thumbnail data";
        let (header, ciphertext) = encrypt_thumbnail(plaintext, &key);

        let result = generate_preview_sync(Some(&ciphertext), Some(&header), &key);

        assert!(!result.is_placeholder);
        assert_eq!(result.thumbnail_bytes, Some(plaintext.to_vec()));
    }

    #[test]
    fn test_generate_preview_sync_no_thumbnail() {
        let key = generate_key();
        let result = generate_preview_sync(None, None, &key);

        assert!(result.is_placeholder);
        assert!(result.thumbnail_bytes.is_none());
    }

    #[test]
    fn test_generate_preview_sync_no_header() {
        let key = generate_key();
        let result = generate_preview_sync(Some(b"data"), None, &key);

        assert!(result.is_placeholder);
        assert!(result.thumbnail_bytes.is_none());
    }

    #[test]
    fn test_generate_preview_sync_wrong_key() {
        let correct_key = generate_key();
        let wrong_key = generate_key();
        let plaintext = b"thumbnail data";
        let (header, ciphertext) = encrypt_thumbnail(plaintext, &correct_key);

        let result = generate_preview_sync(Some(&ciphertext), Some(&header), &wrong_key);

        assert!(result.is_placeholder);
        assert!(result.thumbnail_bytes.is_none());
    }

    #[test]
    fn test_generate_preview_sync_tampered_data() {
        let key = generate_key();
        let plaintext = b"thumbnail data";
        let (header, mut ciphertext) = encrypt_thumbnail(plaintext, &key);
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }

        let result = generate_preview_sync(Some(&ciphertext), Some(&header), &key);

        assert!(result.is_placeholder);
        assert!(result.thumbnail_bytes.is_none());
    }
}
