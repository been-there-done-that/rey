use crate::commands::error::CommandError;
use crate::state::AppState;
use types::device::DeviceInfo;

pub async fn register_device(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<DeviceInfo, CommandError> {
    let device_info = DeviceInfo {
        device_id: uuid::Uuid::new_v4().to_string(),
        name,
        platform: types::device::DevicePlatform::Desktop,
        sse_token: String::new(),
        push_token: None,
        stall_timeout_seconds: 90,
    };

    let mut lock = state.device_info.write().await;
    *lock = Some(device_info.clone());

    Ok(device_info)
}

pub async fn get_device_info(
    state: tauri::State<'_, AppState>,
) -> Result<Option<DeviceInfo>, CommandError> {
    let lock = state.device_info.read().await;
    Ok(lock.clone())
}
