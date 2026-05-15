#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod aead;
pub mod error;
pub mod kdf;
pub mod key;
pub mod seal;
pub mod util;

pub use types::crypto::{Argon2Profile, EncryptedKey, Header24, Key256, Nonce24, Salt16};

#[cfg(test)]
mod prop_tests;
