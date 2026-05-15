use rand_core::OsRng;
use rand_core::RngCore;
use types::crypto::Key256;

pub fn generate_key() -> Key256 {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    Key256::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_produces_32_bytes() {
        let key = generate_key();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn test_generate_key_produces_different_keys() {
        let key1 = generate_key();
        let key2 = generate_key();
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }
}
