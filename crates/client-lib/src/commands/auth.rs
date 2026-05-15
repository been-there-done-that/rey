use crate::commands::error::CommandError;
use crate::state::AppState;
use base64::Engine;
use crypto::Key256;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use types::device::DeviceInfo;

#[derive(Deserialize)]
pub struct LoginParams {
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub session_token: String,
    pub device_info: DeviceInfo,
}

pub async fn get_auth_params(
    state: tauri::State<'_, AppState>,
    email: String,
) -> Result<LoginParams, CommandError> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/params", state.config.server_url);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(zoo_client::ZooError::HttpError)?;

    let result: serde_json::Value = resp.json().await.map_err(zoo_client::ZooError::HttpError)?;
    Ok(LoginParams {
        kek_salt: result["kek_salt"].as_str().unwrap_or("").to_string(),
        mem_limit: result["mem_limit"].as_u64().unwrap_or(0) as u32,
        ops_limit: result["ops_limit"].as_u64().unwrap_or(0) as u32,
    })
}

pub async fn login(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<LoginResponse, CommandError> {
    let params = get_auth_params(state.clone(), email.clone()).await?;

    let salt_bytes = base64::prelude::BASE64_STANDARD
        .decode(&params.kek_salt)
        .map_err(|e| CommandError::Validation(format!("invalid salt: {}", e)))?;

    let mut salt_arr = [0u8; 16];
    salt_arr.copy_from_slice(&salt_bytes);

    let kek = crypto::kdf::argon::derive_key(
        password.as_bytes(),
        &salt_arr,
        params.mem_limit,
        params.ops_limit,
    );

    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/login", state.config.server_url);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email, "verify_key_hash": "" }))
        .send()
        .await
        .map_err(zoo_client::ZooError::HttpError)?;

    let result: serde_json::Value = resp.json().await.map_err(zoo_client::ZooError::HttpError)?;
    let session_token = result["session_token"]
        .as_str()
        .ok_or(CommandError::Validation("missing session_token".to_string()))?
        .to_string();

    state.set_master_key(kek).await;
    state.set_session_token(session_token.clone()).await;

    let device_info = DeviceInfo {
        device_id: uuid::Uuid::new_v4().to_string(),
        name: "Desktop".to_string(),
        platform: types::device::DevicePlatform::Desktop,
        sse_token: String::new(),
        push_token: None,
        stall_timeout_seconds: 90,
    };

    Ok(LoginResponse {
        session_token,
        device_info,
    })
}

pub async fn logout(state: tauri::State<'_, AppState>) -> Result<(), CommandError> {
    state.clear_master_key().await;
    state.clear_session_token().await;
    Ok(())
}

pub async fn register(
    _state: tauri::State<'_, AppState>,
    _email: String,
    _password: String,
) -> Result<(), CommandError> {
    Err(CommandError::Validation("registration not implemented".to_string()))
}
