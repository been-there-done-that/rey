use types::sse::SseEvent;
use types::upload::UploadStatus;
use zoo::sse::events::format_sse;

#[test]
fn test_format_sse_heartbeat() {
    let event = SseEvent::Heartbeat {
        timestamp: 1700000000000,
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.ends_with("\n\n"));
    assert!(formatted.contains("heartbeat"));
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
    assert!(formatted.contains("upload_stalled"));
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
    assert!(formatted.contains("upload_failed"));
    assert!(formatted.contains("upload-2"));
    assert!(formatted.contains("timeout"));
}

#[test]
fn test_format_sse_upload_progress() {
    let event = SseEvent::UploadProgress {
        upload_id: "upload-4".to_string(),
        status: UploadStatus::Uploading,
        parts_bitmask: "AQE=".to_string(),
        part_count: 10,
        device_name: "device-4".to_string(),
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("upload_progress"));
    assert!(formatted.contains("uploading"));
}

#[test]
fn test_format_sse_upload_completed() {
    let event = SseEvent::UploadCompleted {
        upload_id: "upload-5".to_string(),
        device_name: "device-5".to_string(),
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("upload_completed"));
    assert!(formatted.contains("upload-5"));
}

#[test]
fn test_format_sse_upload_done() {
    let event = SseEvent::UploadDone {
        upload_id: "upload-6".to_string(),
        file_id: 100,
        device_name: "device-6".to_string(),
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("upload_done"));
    assert!(formatted.contains("100"));
}

#[test]
fn test_format_sse_device_connected() {
    let event = SseEvent::DeviceConnected {
        device_id: "dev-1".to_string(),
        device_name: "My Phone".to_string(),
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("device_connected"));
    assert!(formatted.contains("dev-1"));
}

#[test]
fn test_format_sse_device_disconnected() {
    let event = SseEvent::DeviceDisconnected {
        device_id: "dev-2".to_string(),
        device_name: "My Laptop".to_string(),
    };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("device_disconnected"));
}

#[test]
fn test_format_sse_upload_pending() {
    let event = SseEvent::UploadPending { uploads: vec![] };
    let formatted = format_sse(&event);
    assert!(formatted.starts_with("data: "));
    assert!(formatted.contains("upload_pending"));
}
