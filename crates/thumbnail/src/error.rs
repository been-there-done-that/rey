use thiserror::Error;

#[derive(Error, Debug)]
pub enum ThumbnailError {
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("thumbnail generation failed: {0}")]
    GenerationFailed(String),
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::error::CryptoError),
    #[error("cache error: {0}")]
    CacheError(String),
    #[error("thumbnail not found")]
    NotFound,
    #[error("download error: {0}")]
    DownloadError(String),
}
