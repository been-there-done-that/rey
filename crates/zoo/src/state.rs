use crate::auth::middleware::AuthState;
use crate::config::ZooConfig;
use crate::sse::hub::SseHub;
use aws_sdk_s3::Client;
use axum::extract::FromRef;
use sqlx::PgPool;
use std::sync::Arc;
use types::error::ApiError;
use types::error::ErrorCode;
use types::upload::UploadStatus;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub pool: PgPool,
    pub s3_client: Client,
    pub config: ZooConfig,
    pub sse_hub: Arc<SseHub>,
}

impl AppState {
    pub fn auth_state(&self) -> AuthState {
        AuthState {
            pool: self.pool.clone(),
        }
    }
}

pub fn validate_transition(from: UploadStatus, to: UploadStatus) -> Result<(), ApiError> {
    let valid = matches!(
        (from, to),
        (UploadStatus::Pending, UploadStatus::Encrypting)
            | (UploadStatus::Pending, UploadStatus::Failed)
            | (UploadStatus::Encrypting, UploadStatus::Uploading)
            | (UploadStatus::Encrypting, UploadStatus::Failed)
            | (UploadStatus::Uploading, UploadStatus::S3Completed)
            | (UploadStatus::Uploading, UploadStatus::Stalled)
            | (UploadStatus::Uploading, UploadStatus::Failed)
            | (UploadStatus::S3Completed, UploadStatus::Registering)
            | (UploadStatus::S3Completed, UploadStatus::Failed)
            | (UploadStatus::Registering, UploadStatus::Done)
            | (UploadStatus::Registering, UploadStatus::Failed)
            | (UploadStatus::Stalled, UploadStatus::Uploading)
            | (UploadStatus::Stalled, UploadStatus::Failed)
    );

    if valid {
        Ok(())
    } else {
        Err(ApiError {
            code: ErrorCode::InvalidStateTransition,
            message: format!("invalid transition from {from:?} to {to:?}"),
            details: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(validate_transition(UploadStatus::Pending, UploadStatus::Encrypting).is_ok());
        assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Uploading).is_ok());
        assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Stalled).is_ok());
        assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Uploading).is_ok());
        assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Failed).is_ok());
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(validate_transition(UploadStatus::Stalled, UploadStatus::Encrypting).is_err());
        assert!(validate_transition(UploadStatus::Done, UploadStatus::Pending).is_err());
        assert!(validate_transition(UploadStatus::Failed, UploadStatus::Uploading).is_err());
    }

    #[test]
    fn test_invalid_transition_error_contains_details() {
        let result = validate_transition(UploadStatus::Done, UploadStatus::Pending);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidStateTransition);
        assert!(err.message.contains("Done"));
        assert!(err.message.contains("Pending"));
        assert!(err.details.is_none());
    }

    #[test]
    fn test_all_pending_transitions() {
        assert!(validate_transition(UploadStatus::Pending, UploadStatus::Encrypting).is_ok());
        assert!(validate_transition(UploadStatus::Pending, UploadStatus::Failed).is_ok());
        assert!(validate_transition(UploadStatus::Pending, UploadStatus::Done).is_err());
        assert!(validate_transition(UploadStatus::Pending, UploadStatus::Pending).is_err());
    }

    #[test]
    fn test_all_encrypting_transitions() {
        assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Uploading).is_ok());
        assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Failed).is_ok());
        assert!(validate_transition(UploadStatus::Encrypting, UploadStatus::Done).is_err());
    }

    #[test]
    fn test_all_uploading_transitions() {
        assert!(validate_transition(UploadStatus::Uploading, UploadStatus::S3Completed).is_ok());
        assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Stalled).is_ok());
        assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Failed).is_ok());
        assert!(validate_transition(UploadStatus::Uploading, UploadStatus::Done).is_err());
    }

    #[test]
    fn test_all_s3_completed_transitions() {
        assert!(validate_transition(UploadStatus::S3Completed, UploadStatus::Registering).is_ok());
        assert!(validate_transition(UploadStatus::S3Completed, UploadStatus::Failed).is_ok());
        assert!(validate_transition(UploadStatus::S3Completed, UploadStatus::Done).is_err());
    }

    #[test]
    fn test_all_registering_transitions() {
        assert!(validate_transition(UploadStatus::Registering, UploadStatus::Done).is_ok());
        assert!(validate_transition(UploadStatus::Registering, UploadStatus::Failed).is_ok());
        assert!(validate_transition(UploadStatus::Registering, UploadStatus::Uploading).is_err());
    }

    #[test]
    fn test_done_is_terminal() {
        assert!(validate_transition(UploadStatus::Done, UploadStatus::Pending).is_err());
        assert!(validate_transition(UploadStatus::Done, UploadStatus::Encrypting).is_err());
        assert!(validate_transition(UploadStatus::Done, UploadStatus::Uploading).is_err());
        assert!(validate_transition(UploadStatus::Done, UploadStatus::Done).is_err());
    }

    #[test]
    fn test_failed_is_terminal() {
        assert!(validate_transition(UploadStatus::Failed, UploadStatus::Pending).is_err());
        assert!(validate_transition(UploadStatus::Failed, UploadStatus::Encrypting).is_err());
        assert!(validate_transition(UploadStatus::Failed, UploadStatus::Uploading).is_err());
        assert!(validate_transition(UploadStatus::Failed, UploadStatus::Failed).is_err());
    }
}
