use types::crypto::{EncryptedKey, Key256};
use crate::aead::secretbox::secretbox_decrypt;
use crate::error::CryptoError;

pub fn decrypt_key(encrypted: &EncryptedKey, wrapping_key: &Key256) -> Result<Key256, CryptoError> {
    let plaintext = secretbox_decrypt(&encrypted.nonce, &encrypted.ciphertext, wrapping_key)?;
    let bytes: [u8; 32] = plaintext
        .try_into()
        .map_err(|_| CryptoError::InvalidKey)?;
    Ok(Key256::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::encrypt::encrypt_key;
    use crate::key::generate_key;

    #[test]
    fn test_decrypt_key_roundtrip() {
        let plaintext = generate_key();
        let wrapping = generate_key();
        let encrypted = encrypt_key(&plaintext, &wrapping);
        let decrypted = decrypt_key(&encrypted, &wrapping).unwrap();
        assert_eq!(plaintext.as_bytes(), decrypted.as_bytes());
    }

    #[test]
    fn test_decrypt_key_wrong_wrapping_key() {
        let plaintext = generate_key();
        let wrapping1 = generate_key();
        let wrapping2 = generate_key();
        let encrypted = encrypt_key(&plaintext, &wrapping1);
        let result = decrypt_key(&encrypted, &wrapping2);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }
}
