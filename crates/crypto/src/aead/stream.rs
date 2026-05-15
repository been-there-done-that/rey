use alloc::vec::Vec;
use aead::{Aead, AeadCore, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use rand_core::OsRng;
use types::crypto::{Header24, Key256};
use crate::error::CryptoError;

pub fn stream_encrypt(plaintext: &[u8], key: &Key256) -> (Header24, Vec<u8>) {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let header = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&header, plaintext).expect("encryption failed");
    (Header24::new(header.into()), ciphertext)
}

pub fn stream_decrypt(header: &Header24, ciphertext: &[u8], key: &Key256) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    cipher
        .decrypt(header.as_bytes().into(), ciphertext)
        .map_err(|_| CryptoError::MacMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::generate_key;

    #[test]
    fn test_stream_roundtrip() {
        let key = generate_key();
        let plaintext = b"file data here";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);
        let decrypted = stream_decrypt(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_stream_mac_mismatch_on_tampered_ciphertext() {
        let key = generate_key();
        let plaintext = b"file data here";
        let (header, mut ciphertext) = stream_encrypt(plaintext, &key);
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = stream_decrypt(&header, &ciphertext, &key);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }

    #[test]
    fn test_stream_wrong_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"file data here";
        let (header, ciphertext) = stream_encrypt(plaintext, &key1);
        let result = stream_decrypt(&header, &ciphertext, &key2);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }

    #[test]
    fn test_stream_empty_plaintext() {
        let key = generate_key();
        let plaintext: &[u8] = b"";
        let (header, ciphertext) = stream_encrypt(plaintext, &key);
        let decrypted = stream_decrypt(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
