use crate::error::CryptoError;
use aead::{Aead, KeyInit};
use alloc::vec::Vec;
use blake2b_simd::Params;
use rand_core::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn seal(plaintext: &[u8], recipient_pk: &PublicKey) -> Vec<u8> {
    let ephemeral_sk = StaticSecret::random_from_rng(OsRng);
    let ephemeral_pk = PublicKey::from(&ephemeral_sk);

    let shared = ephemeral_sk.diffie_hellman(recipient_pk);
    let derived_key = Params::new().hash_length(32).hash(shared.as_bytes());

    let cipher = XSalsa20Poly1305::new(derived_key.as_bytes().into());
    let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .expect("encryption failed");

    let mut output = Vec::with_capacity(32 + nonce.len() + ciphertext.len());
    output.extend_from_slice(&ephemeral_pk.to_bytes());
    output.extend_from_slice(nonce.as_slice());
    output.extend_from_slice(&ciphertext);
    output
}

pub fn open(ciphertext: &[u8], recipient_sk: &StaticSecret) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.len() < 32 + 24 {
        return Err(CryptoError::InvalidKey);
    }

    let ephemeral_pk_bytes: [u8; 32] = ciphertext[..32].try_into().unwrap();
    let ephemeral_pk = PublicKey::from(ephemeral_pk_bytes);

    let shared = recipient_sk.diffie_hellman(&ephemeral_pk);
    let derived_key = Params::new().hash_length(32).hash(shared.as_bytes());

    let nonce_bytes: [u8; 24] = ciphertext[32..56].try_into().unwrap();
    let ct = &ciphertext[56..];

    let cipher = XSalsa20Poly1305::new(derived_key.as_bytes().into());
    cipher
        .decrypt(&nonce_bytes.into(), ct)
        .map_err(|_| CryptoError::MacMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seal::keypair::generate_keypair;

    #[test]
    fn test_seal_open_roundtrip() {
        let (sk, pk) = generate_keypair();
        let plaintext = b"secret message";
        let sealed = seal(plaintext, &pk);
        let opened = open(&sealed, &sk).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_seal_open_wrong_key() {
        let (_sk1, pk1) = generate_keypair();
        let (sk2, _) = generate_keypair();
        let plaintext = b"secret message";
        let sealed = seal(plaintext, &pk1);
        let result = open(&sealed, &sk2);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }

    #[test]
    fn test_seal_open_tampered_ciphertext() {
        let (sk, pk) = generate_keypair();
        let plaintext = b"secret message";
        let mut sealed = seal(plaintext, &pk);
        if sealed.len() > 56 {
            sealed[56] ^= 0xFF;
        }
        let result = open(&sealed, &sk);
        assert!(matches!(result, Err(CryptoError::MacMismatch)));
    }

    #[test]
    fn test_seal_open_empty_plaintext() {
        let (sk, pk) = generate_keypair();
        let plaintext: &[u8] = b"";
        let sealed = seal(plaintext, &pk);
        let opened = open(&sealed, &sk).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn test_seal_open_too_short() {
        let (sk, _) = generate_keypair();
        let result = open(&[0u8; 10], &sk);
        assert!(matches!(result, Err(CryptoError::InvalidKey)));
    }
}
