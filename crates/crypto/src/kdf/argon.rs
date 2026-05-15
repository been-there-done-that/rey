use crate::error::CryptoError;
use argon2::{Algorithm, Argon2, Params, Version};
use types::crypto::{Argon2Profile, Key256, Salt16};

pub fn derive_kek(
    password: &[u8],
    salt: &Salt16,
    profile: Argon2Profile,
) -> Result<Key256, CryptoError> {
    let mut mem = profile.mem_limit();
    let mut ops = profile.ops_limit();
    let floor = 32 * 1024 * 1024; // 32 MiB

    loop {
        let m_cost = (mem / 1024).max(1);
        let t_cost = ops;
        let p_cost = 1;

        let params =
            Params::new(m_cost, t_cost, p_cost, None).map_err(|_| CryptoError::InvalidKey)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut output = [0u8; 32];
        match argon2.hash_password_into(password, salt.as_bytes(), &mut output) {
            Ok(()) => return Ok(Key256::new(output)),
            Err(argon2::Error::MemoryTooMuch) => {
                if mem > floor {
                    mem /= 2;
                    ops *= 2;
                    continue;
                }
                return Err(CryptoError::AllocationFailed);
            }
            Err(_) => return Err(CryptoError::InvalidKey),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::crypto::Salt16;

    #[test]
    fn test_derive_kek_produces_32_bytes() {
        let password = b"test password";
        let salt = Salt16::new([0u8; 16]);
        let profile = Argon2Profile::Interactive;
        let kek = derive_kek(password, &salt, profile).unwrap();
        assert_eq!(kek.as_bytes().len(), 32);
    }

    #[test]
    fn test_derive_kek_deterministic() {
        let password = b"test password";
        let salt = Salt16::new([1u8; 16]);
        let profile = Argon2Profile::Interactive;
        let kek1 = derive_kek(password, &salt, profile).unwrap();
        let kek2 = derive_kek(password, &salt, profile).unwrap();
        assert_eq!(kek1.as_bytes(), kek2.as_bytes());
    }

    #[test]
    fn test_derive_kek_different_passwords() {
        let salt = Salt16::new([0u8; 16]);
        let profile = Argon2Profile::Interactive;
        let kek1 = derive_kek(b"password1", &salt, profile).unwrap();
        let kek2 = derive_kek(b"password2", &salt, profile).unwrap();
        assert_ne!(kek1.as_bytes(), kek2.as_bytes());
    }

    #[test]
    fn test_derive_kek_different_salts() {
        let password = b"test password";
        let profile = Argon2Profile::Interactive;
        let salt1 = Salt16::new([0u8; 16]);
        let salt2 = Salt16::new([1u8; 16]);
        let kek1 = derive_kek(password, &salt1, profile).unwrap();
        let kek2 = derive_kek(password, &salt2, profile).unwrap();
        assert_ne!(kek1.as_bytes(), kek2.as_bytes());
    }
}
