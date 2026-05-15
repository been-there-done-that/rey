use crate::error::ThumbnailError;
use types::crypto::{Header24, Key256};

pub fn decrypt_thumbnail(
    header: &Header24,
    ciphertext: &[u8],
    file_key: &Key256,
) -> Result<Vec<u8>, ThumbnailError> {
    let plaintext =
        crypto::stream_decrypt(header, ciphertext, file_key).map_err(ThumbnailError::Crypto)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;
    use crypto::stream_encrypt;

    #[test]
    fn test_decrypt_thumbnail_roundtrip() {
        let key = generate_key();
        let plaintext = b"thumbnail data here";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);
        let decrypted = decrypt_thumbnail(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_thumbnail_wrong_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"thumbnail data";
        let (header, ciphertext) = stream_encrypt(plaintext, &key1);
        let result = decrypt_thumbnail(&header, &ciphertext, &key2);
        assert!(matches!(result, Err(ThumbnailError::Crypto(_))));
    }

    #[test]
    fn test_decrypt_thumbnail_tampered_ciphertext() {
        let key = generate_key();
        let plaintext = b"thumbnail data";
        let (header, mut ciphertext) = stream_encrypt(plaintext, &key);
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = decrypt_thumbnail(&header, &ciphertext, &key);
        assert!(matches!(result, Err(ThumbnailError::Crypto(_))));
    }

    #[test]
    fn test_decrypt_thumbnail_empty() {
        let key = generate_key();
        let plaintext: &[u8] = b"";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);
        let decrypted = decrypt_thumbnail(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
