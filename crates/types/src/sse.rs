use serde::{Deserialize, Serialize};
use crate::upload::{UploadStatus, UploadSummary};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    UploadProgress {
        upload_id: String,
        status: UploadStatus,
        parts_bitmask: String,
        part_count: u16,
        device_name: String,
    },
    UploadCompleted {
        upload_id: String,
        device_name: String,
    },
    UploadDone {
        upload_id: String,
        file_id: i64,
        device_name: String,
    },
    UploadStalled {
        upload_id: String,
        parts_bitmask: String,
        part_count: u16,
        device_name: String,
        stalled_at: i64,
    },
    UploadFailed {
        upload_id: String,
        reason: String,
        device_name: String,
    },
    UploadPending {
        uploads: Vec<UploadSummary>,
    },
    DeviceConnected {
        device_id: String,
        device_name: String,
    },
    DeviceDisconnected {
        device_id: String,
        device_name: String,
    },
    Heartbeat {
        timestamp: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upload::UploadStatus;

    #[test]
    fn test_sse_event_upload_progress_tag() {
        let event = SseEvent::UploadProgress {
            upload_id: "uuid-1".to_string(),
            status: UploadStatus::Uploading,
            parts_bitmask: "AAAA".to_string(),
            part_count: 4,
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"upload_progress""#));
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::UploadProgress { upload_id, .. } => {
                assert_eq!(upload_id, "uuid-1");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_sse_event_heartbeat_tag() {
        let event = SseEvent::Heartbeat { timestamp: 1700000000000 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"heartbeat""#));
    }

    #[test]
    fn test_sse_event_device_connected_tag() {
        let event = SseEvent::DeviceConnected {
            device_id: "dev-1".to_string(),
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"device_connected""#));
    }

    #[test]
    fn test_sse_event_upload_done_roundtrip() {
        let event = SseEvent::UploadDone {
            upload_id: "uuid-1".to_string(),
            file_id: 42,
            device_name: "My Phone".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::UploadDone { file_id, .. } => assert_eq!(file_id, 42),
            _ => panic!("Wrong variant"),
        }
    }
}
