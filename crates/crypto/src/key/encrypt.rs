use crate::aead::secretbox::secretbox_encrypt;
use types::crypto::{EncryptedKey, Key256};

pub fn encrypt_key(plaintext_key: &Key256, wrapping_key: &Key256) -> EncryptedKey {
    let (nonce, ciphertext) = secretbox_encrypt(plaintext_key.as_bytes(), wrapping_key);
    EncryptedKey { nonce, ciphertext }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::generate_key;

    #[test]
    fn test_encrypt_key_produces_encrypted_key() {
        let plaintext = generate_key();
        let wrapping = generate_key();
        let encrypted = encrypt_key(&plaintext, &wrapping);
        assert!(!encrypted.ciphertext.is_empty());
    }
}
