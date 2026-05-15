use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub encrypted_key: String,
    pub key_nonce: String,
    pub updation_time: i64,
    pub created_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCollection {
    pub id: String,
    pub encrypted_name: String,
    pub name_decryption_nonce: String,
    pub encrypted_key: String,
    pub key_decryption_nonce: String,
    pub updation_time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_roundtrip() {
        let c = Collection {
            id: "col-1".to_string(),
            name: "Vacation Photos".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let decoded: Collection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, c.name);
    }

    #[test]
    fn test_encrypted_collection_roundtrip() {
        let ec = EncryptedCollection {
            id: "col-1".to_string(),
            encrypted_name: "encrypted_name".to_string(),
            name_decryption_nonce: "nonce".to_string(),
            encrypted_key: "ek".to_string(),
            key_decryption_nonce: "kdn".to_string(),
            updation_time: 1700000000000,
        };
        let json = serde_json::to_string(&ec).unwrap();
        let decoded: EncryptedCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.encrypted_name, ec.encrypted_name);
    }
}
