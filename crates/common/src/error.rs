use thiserror::Error;
use crate::config::ConfigError;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

pub fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut output = format!("Error: {err}");

    let mut source = err.source();
    let mut level = 0;
    while let Some(cause) = source {
        if level == 0 {
            output.push_str("\nCaused by:");
        }
        output.push_str(&format!("\n  {level}: {cause}"));
        source = cause.source();
        level += 1;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let common: CommonError = io_err.into();
        match common {
            CommonError::Io(e) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            _ => panic!("Expected CommonError::Io"),
        }
    }

    #[test]
    fn test_serde_error_conversion() {
        let bad_json = "not valid json";
        let result: std::result::Result<serde_json::Value, _> = serde_json::from_str(bad_json);
        let common: CommonError = result.unwrap_err().into();
        match common {
            CommonError::Serde(_) => {}
            _ => panic!("Expected CommonError::Serde"),
        }
    }

    #[test]
    fn test_format_error_chain_single() {
        let err = CommonError::Parse("something went wrong".to_string());
        let output = format_error_chain(&err);
        assert!(output.starts_with("Error: parse error: something went wrong"));
    }

    #[test]
    fn test_format_error_chain_with_cause() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let config_err = ConfigError::IoError(io_err);
        let common: CommonError = config_err.into();

        let output = format_error_chain(&common);
        assert!(output.contains("configuration error"));
        assert!(output.contains("Caused by:"));
        assert!(output.contains("I/O error"));
        assert!(output.contains("access denied"));
    }

    #[test]
    fn test_format_error_chain_multi_level() {
        let io_err = io::Error::new(
            io::ErrorKind::NotFound,
            "No such file or directory",
        );
        let parse_err = CommonError::Parse(format!("config load failed: {io_err}"));

        let output = format_error_chain(&parse_err);
        assert!(output.contains("config load failed"));
    }
}
