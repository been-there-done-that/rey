use proptest::prelude::*;
use crate::aead::{secretbox_decrypt, secretbox_encrypt, stream_decrypt, stream_encrypt};
use crate::key::generate_key;
use crate::seal::{generate_keypair, open, seal};

proptest! {
    #[test]
    fn prop_stream_roundtrip(plaintext in prop::collection::vec(any::<u8>(), 0..512)) {
        let key = generate_key();
        let (header, ciphertext) = stream_encrypt(&plaintext, &key);
        let decrypted = stream_decrypt(&header, &ciphertext, &key).unwrap();
        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn prop_secretbox_roundtrip(plaintext in prop::collection::vec(any::<u8>(), 0..512)) {
        let key = generate_key();
        let (nonce, ciphertext) = secretbox_encrypt(&plaintext, &key);
        let decrypted = secretbox_decrypt(&nonce, &ciphertext, &key).unwrap();
        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn prop_stream_mac_mismatch_on_byte_flip(
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
        byte_idx in any::<usize>(),
    ) {
        let key = generate_key();
        let (header, mut ciphertext) = stream_encrypt(&plaintext, &key);
        let idx = byte_idx % ciphertext.len();
        ciphertext[idx] ^= 0xFF;
        let result = stream_decrypt(&header, &ciphertext, &key);
        prop_assert!(matches!(result, Err(crate::error::CryptoError::MacMismatch)));
    }

    #[test]
    fn prop_sealed_box_roundtrip(plaintext in prop::collection::vec(any::<u8>(), 0..512)) {
        let (sk, pk) = generate_keypair();
        let sealed = seal(&plaintext, &pk);
        let opened = open(&sealed, &sk).unwrap();
        prop_assert_eq!(opened, plaintext);
    }
}
