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
