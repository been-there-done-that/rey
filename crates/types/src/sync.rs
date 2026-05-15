use serde::{Deserialize, Serialize};
use crate::collection::EncryptedCollection;
use crate::file::EncryptedFileRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCollectionResponse {
    pub collections: Vec<EncryptedCollection>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFilesResponse {
    pub updated_files: Vec<EncryptedFileRecord>,
    pub deleted_file_ids: Vec<i64>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTrashResponse {
    pub deleted_files: Vec<DeletedFileRef>,
    pub has_more: bool,
    pub latest_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFileRef {
    pub file_id: i64,
    pub collection_id: String,
    pub updation_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCursor {
    pub key: String,
    pub value: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_collection_response_roundtrip() {
        let resp = SyncCollectionResponse {
            collections: vec![EncryptedCollection {
                id: "col-1".to_string(),
                encrypted_name: "enc".to_string(),
                name_decryption_nonce: "n".to_string(),
                encrypted_key: "ek".to_string(),
                key_decryption_nonce: "kdn".to_string(),
                updation_time: 1700000000000,
            }],
            has_more: false,
            latest_updated_at: 1700000000000,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: SyncCollectionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.has_more, resp.has_more);
        assert_eq!(decoded.collections.len(), 1);
    }

    #[test]
    fn test_deleted_file_ref_roundtrip() {
        let dfr = DeletedFileRef {
            file_id: 42,
            collection_id: "col-1".to_string(),
            updation_time: 1700000000000,
        };
        let json = serde_json::to_string(&dfr).unwrap();
        let decoded: DeletedFileRef = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.file_id, dfr.file_id);
    }

    #[test]
    fn test_sync_cursor_roundtrip() {
        let cursor = SyncCursor {
            key: "updated_at".to_string(),
            value: 1700000000000,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        let decoded: SyncCursor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.key, cursor.key);
    }
}
