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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_mac_mismatch() {
        let err = CryptoError::MacMismatch;
        assert_eq!(format!("{}", err), "MAC verification failed");
    }

    #[test]
    fn test_display_unsupported_cipher() {
        let err = CryptoError::UnsupportedCipher("AES-GCM".to_string());
        assert_eq!(format!("{}", err), "unsupported cipher: AES-GCM");
    }

    #[test]
    fn test_display_allocation_failed() {
        let err = CryptoError::AllocationFailed;
        assert_eq!(
            format!("{}", err),
            "memory allocation failed for Argon2id"
        );
    }

    #[test]
    fn test_display_invalid_key() {
        let err = CryptoError::InvalidKey;
        assert_eq!(format!("{}", err), "invalid key length");
    }

    #[test]
    fn test_display_invalid_nonce() {
        let err = CryptoError::InvalidNonce;
        assert_eq!(format!("{}", err), "invalid nonce length");
    }

    #[test]
    fn test_display_base64_error() {
        let err = CryptoError::Base64Error("invalid input".to_string());
        assert_eq!(format!("{}", err), "base64 decode error: invalid input");
    }

    #[test]
    fn test_display_hex_error() {
        let err = CryptoError::HexError("bad hex".to_string());
        assert_eq!(format!("{}", err), "hex decode error: bad hex");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_error_source_returns_none() {
        let err = CryptoError::MacMismatch;
        assert!(std::error::Error::source(&err).is_none());

        let err = CryptoError::UnsupportedCipher("test".to_string());
        assert!(std::error::Error::source(&err).is_none());
    }
}
