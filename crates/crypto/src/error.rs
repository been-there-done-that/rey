use alloc::string::String;
use core::fmt;

#[derive(Debug)]
pub enum CryptoError {
    MacMismatch,
    UnsupportedCipher(String),
    AllocationFailed,
    InvalidKey,
    InvalidNonce,
    Base64Error(String),
    HexError(String),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::MacMismatch => write!(f, "MAC verification failed"),
            CryptoError::UnsupportedCipher(s) => write!(f, "unsupported cipher: {s}"),
            CryptoError::AllocationFailed => write!(f, "memory allocation failed for Argon2id"),
            CryptoError::InvalidKey => write!(f, "invalid key length"),
            CryptoError::InvalidNonce => write!(f, "invalid nonce length"),
            CryptoError::Base64Error(s) => write!(f, "base64 decode error: {s}"),
            CryptoError::HexError(s) => write!(f, "hex decode error: {s}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CryptoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
