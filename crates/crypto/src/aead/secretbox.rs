use crate::error::CryptoError;
use aead::{Aead, KeyInit};
use alloc::vec::Vec;
use rand_core::OsRng;
use types::crypto::{Key256, Nonce24};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn secretbox_encrypt(plaintext: &[u8], key: &Key256) -> (Nonce24, Vec<u8>) {
    let cipher = XSalsa20Poly1305::new(key.as_bytes().into());
    let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .expect("encryption failed");
    (Nonce24::new(nonce.into()), ciphertext)
}

pub fn secretbox_decrypt(
    nonce: &Nonce24,
    ciphertext: &[u8],
    key: &Key256,
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XSalsa20Poly1305::new(key.as_bytes().into());
    cipher
        .decrypt(nonce.as_bytes().into(), ciphertext)
        .map_err(|_| CryptoError::MacMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::generate_key;

    #[test]
    fn test_secretbox_roundtrip() {
        let key = generate_key();
        let plaintext = b"hello world";
        let (nonce, ciphertext) = secretbox_encrypt(plaintext, &key);
        let decrypted = secretbox_decrypt(&nonce, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_secretbox_mac_mismatch_on_tampered_ciphertext() {
        let key = generate_key();
        let plaintext = b"hello world";
        let (nonce, mut ciphertext) = secretbox_encrypt(plaintext, &key);
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = secretbox_decrypt(&nonce, &ciphertext, &key);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }

    #[test]
    fn test_secretbox_wrong_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"hello world";
        let (nonce, ciphertext) = secretbox_encrypt(plaintext, &key1);
        let result = secretbox_decrypt(&nonce, &ciphertext, &key2);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }
}
