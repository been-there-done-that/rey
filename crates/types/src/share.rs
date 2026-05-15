use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    pub file_id: i64,
    pub shared_with: String,
    pub collection_id: String,
    pub encrypted_collection_key: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRequest {
    pub file_id: i64,
    pub shared_with: String,
    pub collection_id: String,
    pub encrypted_collection_key: String,
    pub expires_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_record_roundtrip() {
        let sr = ShareRecord {
            file_id: 1,
            shared_with: "user-2".to_string(),
            collection_id: "col-1".to_string(),
            encrypted_collection_key: "eck".to_string(),
            created_at: 1700000000000,
            expires_at: Some(1700086400000),
        };
        let json = serde_json::to_string(&sr).unwrap();
        let decoded: ShareRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.shared_with, sr.shared_with);
    }

    #[test]
    fn test_share_request_roundtrip() {
        let req = ShareRequest {
            file_id: 1,
            shared_with: "user-2".to_string(),
            collection_id: "col-1".to_string(),
            encrypted_collection_key: "eck".to_string(),
            expires_at: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ShareRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.file_id, req.file_id);
    }
}
