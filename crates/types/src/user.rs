use serde::{Deserialize, Serialize};
use crate::crypto::KeyAttributes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegistration {
    pub email: String,
    pub verify_key_hash: String,
    pub encrypted_master_key: String,
    pub key_nonce: String,
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
    pub public_key: String,
    pub encrypted_secret_key: String,
    pub secret_key_nonce: String,
    pub encrypted_recovery_key: String,
    pub recovery_key_nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginParams {
    pub kek_salt: String,
    pub mem_limit: u32,
    pub ops_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub verify_key_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub session_token: String,
    pub key_attributes: KeyAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub user_id: String,
    pub expires_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyAttributes;

    #[test]
    fn test_user_registration_roundtrip() {
        let ur = UserRegistration {
            email: "test@example.com".to_string(),
            verify_key_hash: "vkh".to_string(),
            encrypted_master_key: "emk".to_string(),
            key_nonce: "kn".to_string(),
            kek_salt: "salt".to_string(),
            mem_limit: 256 * 1024 * 1024,
            ops_limit: 4,
            public_key: "pk".to_string(),
            encrypted_secret_key: "esk".to_string(),
            secret_key_nonce: "skn".to_string(),
            encrypted_recovery_key: "erk".to_string(),
            recovery_key_nonce: "rkn".to_string(),
        };
        let json = serde_json::to_string(&ur).unwrap();
        let decoded: UserRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.email, ur.email);
    }

    #[test]
    fn test_login_params_roundtrip() {
        let lp = LoginParams {
            kek_salt: "salt".to_string(),
            mem_limit: 128 * 1024 * 1024,
            ops_limit: 3,
        };
        let json = serde_json::to_string(&lp).unwrap();
        let decoded: LoginParams = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mem_limit, lp.mem_limit);
    }

    #[test]
    fn test_login_response_roundtrip() {
        let lr = LoginResponse {
            session_token: "token".to_string(),
            key_attributes: KeyAttributes {
                encrypted_master_key: "emk".to_string(),
                key_nonce: "kn".to_string(),
                kek_salt: "salt".to_string(),
                mem_limit: 256 * 1024 * 1024,
                ops_limit: 4,
            },
        };
        let json = serde_json::to_string(&lr).unwrap();
        let decoded: LoginResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.session_token, lr.session_token);
    }

    #[test]
    fn test_session_info_roundtrip() {
        let si = SessionInfo {
            user_id: "user-1".to_string(),
            expires_at: 1700086400000,
        };
        let json = serde_json::to_string(&si).unwrap();
        let decoded: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.user_id, si.user_id);
    }
}
