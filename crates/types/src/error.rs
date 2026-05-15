use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    NotFound,
    UploadAlreadyExists,
    InvalidStateTransition,
    DeviceNameTaken,
    ValidationError,
    FileTooLarge,
    PartCountExceeded,
    SizeMismatch,
    RateLimited,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serialization() {
        assert_eq!(serde_json::to_string(&ErrorCode::Unauthorized).unwrap(), "\"unauthorized\"");
        assert_eq!(serde_json::to_string(&ErrorCode::NotFound).unwrap(), "\"not_found\"");
        assert_eq!(serde_json::to_string(&ErrorCode::UploadAlreadyExists).unwrap(), "\"upload_already_exists\"");
        assert_eq!(serde_json::to_string(&ErrorCode::InvalidStateTransition).unwrap(), "\"invalid_state_transition\"");
        assert_eq!(serde_json::to_string(&ErrorCode::RateLimited).unwrap(), "\"rate_limited\"");
    }

    #[test]
    fn test_api_error_roundtrip() {
        let err = ApiError {
            code: ErrorCode::ValidationError,
            message: "Invalid email".to_string(),
            details: Some(serde_json::json!({"field": "email"})),
        };
        let json = serde_json::to_string(&err).unwrap();
        let decoded: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.code, err.code);
        assert_eq!(decoded.message, err.message);
    }

    #[test]
    fn test_error_response_roundtrip() {
        let resp = ErrorResponse {
            error: ApiError {
                code: ErrorCode::InternalError,
                message: "Something went wrong".to_string(),
                details: None,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.error.code, resp.error.code);
    }
}
