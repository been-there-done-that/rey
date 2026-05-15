use types::crypto::{Header24, Key256};

pub fn encrypt_thumbnail(bytes: &[u8], file_key: &Key256) -> (Header24, Vec<u8>) {
    crypto::stream_encrypt(bytes, file_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::key::generate_key;
    use crypto::stream_decrypt;

    #[test]
    fn test_encrypt_thumbnail_roundtrip() {
        let key = generate_key();
        let plaintext = b"thumbnail bytes";
        let (header, ciphertext) = encrypt_thumbnail(plaintext, &key);
        let decrypted = stream_decrypt(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_thumbnail_produces_different_nonce() {
        let key = generate_key();
        let plaintext = b"same data";
        let (header1, _) = encrypt_thumbnail(plaintext, &key);
        let (header2, _) = encrypt_thumbnail(plaintext, &key);
        assert_ne!(header1.as_bytes(), header2.as_bytes());
    }

    #[test]
    fn test_encrypt_thumbnail_empty() {
        let key = generate_key();
        let (header, ciphertext) = encrypt_thumbnail(b"", &key);
        let decrypted = stream_decrypt(&header, &ciphertext, &key).unwrap();
        assert!(decrypted.is_empty());
    }
}
