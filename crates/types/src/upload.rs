use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    Pending,
    Encrypting,
    Uploading,
    S3Completed,
    Registering,
    Done,
    Stalled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadState {
    pub upload_id: String,
    pub user_id: String,
    pub device_id: String,
    pub status: UploadStatus,
    pub file_hash: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub part_size: i32,
    pub part_count: u16,
    pub parts_bitmask: String,
    pub object_key: Option<String>,
    pub upload_id_s3: Option<String>,
    pub complete_url: Option<String>,
    pub urls_expire_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub stalled_at: Option<i64>,
    pub error_reason: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub done_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartRecord {
    pub part_number: u16,
    pub part_size: i32,
    pub part_md5: String,
    pub etag: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSummary {
    pub upload_id: String,
    pub status: UploadStatus,
    pub file_hash: String,
    pub file_size: i64,
    pub part_count: u16,
    pub parts_completed: u16,
    pub device_name: String,
    pub stalled_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_status_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&UploadStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Encrypting).unwrap(),
            "\"encrypting\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Uploading).unwrap(),
            "\"uploading\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::S3Completed).unwrap(),
            "\"s3_completed\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Registering).unwrap(),
            "\"registering\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Done).unwrap(),
            "\"done\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Stalled).unwrap(),
            "\"stalled\""
        );
        assert_eq!(
            serde_json::to_string(&UploadStatus::Failed).unwrap(),
            "\"failed\""
        );
    }

    #[test]
    fn test_upload_status_deserializes_from_snake_case() {
        assert_eq!(
            serde_json::from_str::<UploadStatus>("\"pending\"").unwrap(),
            UploadStatus::Pending
        );
        assert_eq!(
            serde_json::from_str::<UploadStatus>("\"s3_completed\"").unwrap(),
            UploadStatus::S3Completed
        );
    }

    #[test]
    fn test_upload_state_roundtrip() {
        let us = UploadState {
            upload_id: "uuid-1".to_string(),
            user_id: "user-1".to_string(),
            device_id: "dev-1".to_string(),
            status: UploadStatus::Uploading,
            file_hash: "hash".to_string(),
            file_size: 1024,
            mime_type: Some("image/jpeg".to_string()),
            part_size: 5 * 1024 * 1024,
            part_count: 4,
            parts_bitmask: "AAAA".to_string(),
            object_key: Some("obj/key".to_string()),
            upload_id_s3: Some("s3-id".to_string()),
            complete_url: Some("http://example.com/complete".to_string()),
            urls_expire_at: Some(1700000000000),
            last_heartbeat_at: Some(1700000000000),
            stalled_at: None,
            error_reason: None,
            created_at: 1700000000000,
            expires_at: 1700003600000,
            done_at: None,
        };
        let json = serde_json::to_string(&us).unwrap();
        let decoded: UploadState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, us.status);
        assert_eq!(decoded.part_count, us.part_count);
    }

    #[test]
    fn test_part_record_roundtrip() {
        let pr = PartRecord {
            part_number: 1,
            part_size: 5242880,
            part_md5: "abc123".to_string(),
            etag: Some("etag456".to_string()),
            status: "uploaded".to_string(),
        };
        let json = serde_json::to_string(&pr).unwrap();
        let decoded: PartRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.part_number, pr.part_number);
    }

    #[test]
    fn test_upload_summary_roundtrip() {
        let summary = UploadSummary {
            upload_id: "uuid-1".to_string(),
            status: UploadStatus::Done,
            file_hash: "hash".to_string(),
            file_size: 1024,
            part_count: 2,
            parts_completed: 2,
            device_name: "My Phone".to_string(),
            stalled_at: None,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let decoded: UploadSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, summary.status);
    }
}
