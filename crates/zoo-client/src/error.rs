#[derive(thiserror::Error, Debug)]
pub enum ZooError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("S3 error: {0}")]
    S3Error(String),

    #[error("upload was aborted by GC or manual action")]
    UploadAborted,

    #[error("invalid state transition: {0}")]
    StateError(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("not authenticated")]
    NotAuthenticated,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("upload not found: {0}")]
    UploadNotFound(String),

    #[error("network unavailable")]
    NetworkUnavailable,
}
