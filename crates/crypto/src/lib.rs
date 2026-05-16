#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod aead;
pub mod error;
pub mod kdf;
pub mod key;
pub mod seal;
pub mod util;

pub use aead::{secretbox_decrypt, secretbox_encrypt, stream_decrypt, stream_encrypt};
pub use kdf::{derive_kek, derive_verification_key, derive_subkey};
pub use key::{encrypt_key, decrypt_key, generate_key};
pub use seal::{seal, open, generate_keypair};
pub use types::crypto::{Argon2Profile, EncryptedKey, Header24, Key256, Nonce24, Salt16};

#[cfg(test)]
mod prop_tests;
