use crate::error::ZooError;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024; // 10 GiB
const MIN_PART_SIZE: u32 = 5 * 1024 * 1024; // 5 MiB
const MAX_PART_SIZE: u64 = 5 * 1024 * 1024 * 1024; // 5 GiB
const MAX_PART_COUNT: u16 = 10000;
const MAX_EMAIL_LENGTH: usize = 255;
const MAX_DEVICE_NAME_LENGTH: usize = 64;

pub fn validate_file_size(size: u64) -> Result<(), ZooError> {
    if size > MAX_FILE_SIZE {
        Err(ZooError::FileTooLarge)
    } else {
        Ok(())
    }
}

pub fn validate_part_size(size: u64) -> Result<(), ZooError> {
    if size < MIN_PART_SIZE as u64 || size > MAX_PART_SIZE {
        Err(ZooError::Validation(format!(
            "part size must be between {} and {}",
            MIN_PART_SIZE, MAX_PART_SIZE
        )))
    } else {
        Ok(())
    }
}

pub fn validate_part_count(count: u16) -> Result<(), ZooError> {
    if count > MAX_PART_COUNT {
        Err(ZooError::PartCountExceeded)
    } else {
        Ok(())
    }
}

pub fn validate_part_md5s(md5s: &[String], expected_count: u16) -> Result<(), ZooError> {
    if md5s.len() != expected_count as usize {
        return Err(ZooError::Validation(
            "part_md5s count does not match part_count".to_string(),
        ));
    }
    for md5 in md5s {
        if md5.len() != 32 {
            return Err(ZooError::Validation(format!(
                "invalid MD5 length: {}",
                md5.len()
            )));
        }
    }
    Ok(())
}

pub fn validate_email(email: &str) -> Result<(), ZooError> {
    if email.is_empty() || email.len() > MAX_EMAIL_LENGTH {
        return Err(ZooError::Validation("invalid email".to_string()));
    }
    if !email.contains('@') {
        return Err(ZooError::Validation("invalid email".to_string()));
    }
    Ok(())
}

pub fn validate_device_name(name: &str) -> Result<(), ZooError> {
    if name.is_empty() || name.len() > MAX_DEVICE_NAME_LENGTH {
        return Err(ZooError::Validation("invalid device name".to_string()));
    }
    if name.contains('\0') {
        return Err(ZooError::Validation("device name contains null bytes".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_size() {
        assert!(validate_file_size(1024).is_ok());
        assert!(validate_file_size(MAX_FILE_SIZE + 1).is_err());
    }

    #[test]
    fn test_validate_part_size() {
        assert!(validate_part_size(MIN_PART_SIZE).is_ok());
        assert!(validate_part_size(MIN_PART_SIZE - 1).is_err());
        assert!(validate_part_size(MAX_PART_SIZE + 1).is_err());
    }

    #[test]
    fn test_validate_part_count() {
        assert!(validate_part_count(1).is_ok());
        assert!(validate_part_count(MAX_PART_COUNT).is_ok());
        assert!(validate_part_count(MAX_PART_COUNT + 1).is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("").is_err());
    }

    #[test]
    fn test_validate_device_name() {
        assert!(validate_device_name("My Phone").is_ok());
        assert!(validate_device_name("").is_err());
        assert!(validate_device_name("name\0with\0null").is_err());
    }
}
