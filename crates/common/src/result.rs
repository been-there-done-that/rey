use crate::error::CommonError;

pub type Result<T> = std::result::Result<T, CommonError>;

pub trait ResultExt<T, E> {
    fn context(self, msg: &str) -> std::result::Result<T, CommonError>;

    fn with_context<F>(self, f: F) -> std::result::Result<T, CommonError>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, msg: &str) -> std::result::Result<T, CommonError> {
        self.map_err(|e| CommonError::Parse(format!("{msg}: {e}")))
    }

    fn with_context<F>(self, f: F) -> std::result::Result<T, CommonError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| CommonError::Parse(format!("{}: {e}", f())))
    }
}

pub trait CommonResultExt<T> {
    fn context(self, msg: &str) -> Result<T>;
}

impl<T> CommonResultExt<T> for Result<T> {
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| CommonError::Parse(format!("{msg}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_result_ext_context_on_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let result: std::result::Result<(), _> = Err(io_err);
        let wrapped = result.context("failed to read config");

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("failed to read config"));
                assert!(msg.contains("file missing"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }

    #[test]
    fn test_result_ext_context_on_ok() {
        let result: std::result::Result<i32, io::Error> = Ok(42);
        let wrapped = result.context("this should not be used");
        assert_eq!(wrapped.unwrap(), 42);
    }

    #[test]
    fn test_result_ext_with_context_lazy() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let result: std::result::Result<(), _> = Err(io_err);
        let wrapped = result.with_context(|| format!("accessing {}", "/secret/path"));

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("accessing /secret/path"));
                assert!(msg.contains("denied"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }

    #[test]
    fn test_common_result_ext_context() {
        let err: Result<i32> = Err(CommonError::Parse("original error".to_string()));
        let wrapped = CommonResultExt::context(err, "outer context");

        match wrapped {
            Err(CommonError::Parse(msg)) => {
                assert!(msg.contains("outer context"));
                assert!(msg.contains("original error"));
            }
            _ => panic!("Expected CommonError::Parse"),
        }
    }
}
