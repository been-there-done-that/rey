use blake2b_simd::{Hash, Params};
use types::crypto::Key256;

pub fn derive_verification_key(kek: &Key256) -> Key256 {
    let mut params = Params::new();
    params.hash_length(32);
    params.key(kek.as_bytes());
    params.personal(b"verification");
    let hash: Hash = params.hash(b"");
    let bytes: [u8; 32] = hash
        .as_bytes()
        .try_into()
        .expect("BLAKE2b output is 32 bytes");
    Key256::new(bytes)
}

pub fn derive_subkey(master: &Key256, context: &str, id: u64) -> Key256 {
    let mut personal = [0u8; 8];
    let context_bytes = context.as_bytes();
    let len = context_bytes.len().min(8);
    personal[..len].copy_from_slice(&context_bytes[..len]);

    let mut params = Params::new();
    params.hash_length(32);
    params.key(master.as_bytes());
    params.personal(&personal);

    let mut data = [0u8; 8];
    data.copy_from_slice(&id.to_le_bytes());
    let hash: Hash = params.hash(&data);
    let bytes: [u8; 32] = hash
        .as_bytes()
        .try_into()
        .expect("BLAKE2b output is 32 bytes");
    Key256::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_verification_key_deterministic() {
        let kek = Key256::new([42u8; 32]);
        let vk1 = derive_verification_key(&kek);
        let vk2 = derive_verification_key(&kek);
        assert_eq!(vk1.as_bytes(), vk2.as_bytes());
    }

    #[test]
    fn test_derive_verification_key_different_keys() {
        let kek1 = Key256::new([1u8; 32]);
        let kek2 = Key256::new([2u8; 32]);
        let vk1 = derive_verification_key(&kek1);
        let vk2 = derive_verification_key(&kek2);
        assert_ne!(vk1.as_bytes(), vk2.as_bytes());
    }

    #[test]
    fn test_derive_subkey_deterministic() {
        let master = Key256::new([99u8; 32]);
        let sk1 = derive_subkey(&master, "file", 1);
        let sk2 = derive_subkey(&master, "file", 1);
        assert_eq!(sk1.as_bytes(), sk2.as_bytes());
    }

    #[test]
    fn test_derive_subkey_different_contexts() {
        let master = Key256::new([99u8; 32]);
        let sk1 = derive_subkey(&master, "file", 1);
        let sk2 = derive_subkey(&master, "thumb", 1);
        assert_ne!(sk1.as_bytes(), sk2.as_bytes());
    }

    #[test]
    fn test_derive_subkey_different_ids() {
        let master = Key256::new([99u8; 32]);
        let sk1 = derive_subkey(&master, "file", 1);
        let sk2 = derive_subkey(&master, "file", 2);
        assert_ne!(sk1.as_bytes(), sk2.as_bytes());
    }
}
