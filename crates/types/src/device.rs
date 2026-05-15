use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevicePlatform {
    Android,
    Ios,
    Web,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: String,
    pub platform: DevicePlatform,
    pub sse_token: String,
    pub push_token: Option<String>,
    pub stall_timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub name: String,
    pub platform: DevicePlatform,
    pub push_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_platform_serialization() {
        assert_eq!(serde_json::to_string(&DevicePlatform::Android).unwrap(), "\"android\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Ios).unwrap(), "\"ios\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Web).unwrap(), "\"web\"");
        assert_eq!(serde_json::to_string(&DevicePlatform::Desktop).unwrap(), "\"desktop\"");
    }

    #[test]
    fn test_device_info_roundtrip() {
        let di = DeviceInfo {
            device_id: "dev-1".to_string(),
            name: "My Phone".to_string(),
            platform: DevicePlatform::Android,
            sse_token: "token".to_string(),
            push_token: Some("push-token".to_string()),
            stall_timeout_seconds: 300,
        };
        let json = serde_json::to_string(&di).unwrap();
        let decoded: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, di.name);
        assert_eq!(decoded.platform, di.platform);
    }

    #[test]
    fn test_device_registration_roundtrip() {
        let dr = DeviceRegistration {
            name: "New Device".to_string(),
            platform: DevicePlatform::Desktop,
            push_token: None,
        };
        let json = serde_json::to_string(&dr).unwrap();
        let decoded: DeviceRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, dr.name);
    }
}
