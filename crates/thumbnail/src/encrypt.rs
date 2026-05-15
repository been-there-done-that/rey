use types::crypto::{Header24, Key256};

pub fn encrypt_thumbnail(bytes: &[u8], file_key: &Key256) -> (Header24, Vec<u8>) {
    crypto::stream_encrypt(bytes, file_key)
}
