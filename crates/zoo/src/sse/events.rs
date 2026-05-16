use types::sse::SseEvent;

pub fn format_sse(event: &SseEvent) -> String {
    let json = serde_json::to_string(event).unwrap_or_default();
    format!("data: {json}\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::sse::SseEvent;

    #[test]
    fn test_format_sse_heartbeat() {
        let event = SseEvent::Heartbeat {
            timestamp: 1700000000000,
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.ends_with("\n\n"));
        assert!(formatted.contains("Heartbeat"));
        assert!(formatted.contains("1700000000000"));
    }

    #[test]
    fn test_format_sse_upload_stalled() {
        let event = SseEvent::UploadStalled {
            upload_id: "upload-1".to_string(),
            parts_bitmask: "AQE=".to_string(),
            part_count: 3,
            device_name: "device-1".to_string(),
            stalled_at: 1700000000000,
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("UploadStalled"));
        assert!(formatted.contains("upload-1"));
        assert!(formatted.contains("device-1"));
    }

    #[test]
    fn test_format_sse_upload_failed() {
        let event = SseEvent::UploadFailed {
            upload_id: "upload-2".to_string(),
            reason: "timeout".to_string(),
            device_name: "device-2".to_string(),
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("UploadFailed"));
        assert!(formatted.contains("upload-2"));
        assert!(formatted.contains("timeout"));
    }

    #[test]
    fn test_format_sse_file_uploaded() {
        let event = SseEvent::FileUploaded {
            file_id: 42,
            upload_id: "upload-3".to_string(),
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("FileUploaded"));
        assert!(formatted.contains("42"));
    }

    #[test]
    fn test_format_sse_file_registered() {
        let event = SseEvent::FileRegistered {
            file_id: 99,
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("FileRegistered"));
        assert!(formatted.contains("99"));
    }

    #[test]
    fn test_format_sse_upload_progress() {
        let event = SseEvent::UploadProgress {
            upload_id: "upload-4".to_string(),
            parts_completed: 5,
            total_parts: 10,
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("UploadProgress"));
        assert!(formatted.contains("5"));
        assert!(formatted.contains("10"));
    }

    #[test]
    fn test_format_sse_upload_completed() {
        let event = SseEvent::UploadCompleted {
            upload_id: "upload-5".to_string(),
            file_id: 100,
        };
        let formatted = format_sse(&event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.contains("UploadCompleted"));
        assert!(formatted.contains("100"));
    }
}
