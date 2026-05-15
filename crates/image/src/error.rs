use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImageError {
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("failed to decode image: {0}")]
    DecodeError(String),
    #[error("failed to parse EXIF: {0}")]
    ExifError(String),
}
