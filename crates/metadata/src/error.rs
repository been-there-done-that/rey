use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::error::CryptoError),
    #[error("JSON serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid UTF-8 in metadata")]
    InvalidUtf8,
}
