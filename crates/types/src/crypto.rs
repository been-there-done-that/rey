use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct Key256([u8; 32]);

impl Key256 {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nonce24([u8; 24]);

impl Nonce24 {
    pub fn new(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Header24([u8; 24]);

impl Header24 {
    pub fn new(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Salt16([u8; 16]);

impl Salt16 {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedKey {
    pub nonce: Nonce24,
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyAttributes {
    pub encrypted_master_key: String,
    pub key_nonce: String,
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argon2Profile {
    Sensitive,
    Mobile,
    Interactive,
}

impl Argon2Profile {
    pub fn mem_limit(&self) -> u32 {
        match self {
            Argon2Profile::Sensitive => 256 * 1024 * 1024,
            Argon2Profile::Mobile => 128 * 1024 * 1024,
            Argon2Profile::Interactive => 64 * 1024 * 1024,
        }
    }

    pub fn ops_limit(&self) -> u32 {
        match self {
            Argon2Profile::Sensitive => 4,
            Argon2Profile::Mobile => 3,
            Argon2Profile::Interactive => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_profile_mem_limit_sensitive() {
        assert_eq!(Argon2Profile::Sensitive.mem_limit(), 256 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_mem_limit_mobile() {
        assert_eq!(Argon2Profile::Mobile.mem_limit(), 128 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_mem_limit_interactive() {
        assert_eq!(Argon2Profile::Interactive.mem_limit(), 64 * 1024 * 1024);
    }

    #[test]
    fn test_argon2_profile_ops_limit_sensitive() {
        assert_eq!(Argon2Profile::Sensitive.ops_limit(), 4);
    }

    #[test]
    fn test_argon2_profile_ops_limit_mobile() {
        assert_eq!(Argon2Profile::Mobile.ops_limit(), 3);
    }

    #[test]
    fn test_argon2_profile_ops_limit_interactive() {
        assert_eq!(Argon2Profile::Interactive.ops_limit(), 2);
    }

    #[test]
    fn test_key_attributes_roundtrip() {
        let ka = KeyAttributes {
            encrypted_master_key: "base64key".to_string(),
            key_nonce: "nonce123".to_string(),
            kek_salt: "salt456".to_string(),
            mem_limit: 256 * 1024 * 1024,
            ops_limit: 4,
        };
        let json = serde_json::to_string(&ka).unwrap();
        let decoded: KeyAttributes = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.encrypted_master_key, ka.encrypted_master_key);
        assert_eq!(decoded.mem_limit, ka.mem_limit);
    }

    #[test]
    fn test_encrypted_key_roundtrip() {
        let ek = EncryptedKey {
            nonce: Nonce24::new([0u8; 24]),
            ciphertext: vec![1, 2, 3, 4],
        };
        let json = serde_json::to_string(&ek).unwrap();
        let decoded: EncryptedKey = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.ciphertext, ek.ciphertext);
    }
}
