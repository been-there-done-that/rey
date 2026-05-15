use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZooError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("upload already exists")]
    UploadAlreadyExists,
    #[error("invalid state transition")]
    InvalidStateTransition,
    #[error("device name taken")]
    DeviceNameTaken,
    #[error("file too large")]
    FileTooLarge,
    #[error("part count exceeded")]
    PartCountExceeded,
    #[error("size mismatch")]
    SizeMismatch,
    #[error("rate limited")]
    RateLimited,
    #[error("internal error: {0}")]
    Internal(String),
    #[error("S3 error: {0}")]
    S3(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_found() {
        let err = ZooError::NotFound;
        assert_eq!(format!("{}", err), "not found");
    }

    #[test]
    fn test_display_unauthorized() {
        let err = ZooError::Unauthorized;
        assert_eq!(format!("{}", err), "unauthorized");
    }

    #[test]
    fn test_display_forbidden() {
        let err = ZooError::Forbidden;
        assert_eq!(format!("{}", err), "forbidden");
    }

    #[test]
    fn test_display_validation() {
        let err = ZooError::Validation("bad input".to_string());
        assert_eq!(format!("{}", err), "validation error: bad input");
    }

    #[test]
    fn test_display_upload_already_exists() {
        let err = ZooError::UploadAlreadyExists;
        assert_eq!(format!("{}", err), "upload already exists");
    }

    #[test]
    fn test_display_invalid_state_transition() {
        let err = ZooError::InvalidStateTransition;
        assert_eq!(format!("{}", err), "invalid state transition");
    }

    #[test]
    fn test_display_device_name_taken() {
        let err = ZooError::DeviceNameTaken;
        assert_eq!(format!("{}", err), "device name taken");
    }

    #[test]
    fn test_display_file_too_large() {
        let err = ZooError::FileTooLarge;
        assert_eq!(format!("{}", err), "file too large");
    }

    #[test]
    fn test_display_part_count_exceeded() {
        let err = ZooError::PartCountExceeded;
        assert_eq!(format!("{}", err), "part count exceeded");
    }

    #[test]
    fn test_display_size_mismatch() {
        let err = ZooError::SizeMismatch;
        assert_eq!(format!("{}", err), "size mismatch");
    }

    #[test]
    fn test_display_rate_limited() {
        let err = ZooError::RateLimited;
        assert_eq!(format!("{}", err), "rate limited");
    }

    #[test]
    fn test_display_internal() {
        let err = ZooError::Internal("something broke".to_string());
        assert_eq!(format!("{}", err), "internal error: something broke");
    }

    #[test]
    fn test_display_s3() {
        let err = ZooError::S3("connection refused".to_string());
        assert_eq!(format!("{}", err), "S3 error: connection refused");
    }

    #[test]
    fn test_debug_format() {
        let err = ZooError::NotFound;
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
    }

    #[test]
    fn test_error_source_for_database() {
        let sqlx_err = sqlx::Error::Protocol("test".to_string());
        let zoo_err = ZooError::Database(sqlx_err);
        assert!(std::error::Error::source(&zoo_err).is_some());
    }

    #[test]
    fn test_error_source_for_simple_errors() {
        let err = ZooError::NotFound;
        assert!(std::error::Error::source(&err).is_none());

        let err = ZooError::Validation("test".to_string());
        assert!(std::error::Error::source(&err).is_none());
    }
}
