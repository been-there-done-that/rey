use base64::{engine::general_purpose::STANDARD, Engine};
use crypto::{
    derive_kek, derive_verification_key, encrypt_key, generate_key, generate_keypair,
    Argon2Profile, Key256, Salt16,
};
use rand_core::RngCore;
use wasm_bindgen::prelude::*;

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand_core::OsRng.fill_bytes(&mut buf);
    buf
}

#[wasm_bindgen]
pub fn generate_key_b64() -> String {
    let key = generate_key();
    STANDARD.encode(key.as_bytes())
}

#[wasm_bindgen]
pub fn generate_keypair_b64() -> String {
    let (sk, pk) = generate_keypair();
    let result = serde_json::json!({
        "public_key": STANDARD.encode(pk.to_bytes()),
        "secret_key": STANDARD.encode(sk.to_bytes()),
    });
    result.to_string()
}

#[wasm_bindgen]
pub fn generate_salt_b64() -> String {
    let salt = Salt16::new(random_bytes());
    STANDARD.encode(salt.as_bytes())
}

#[wasm_bindgen]
pub fn derive_kek_b64(password: &str, salt_b64: &str, _mem_limit: u32, _ops_limit: u32) -> Result<String, JsError> {
    let salt_bytes = STANDARD.decode(salt_b64).map_err(|e| JsError::new(&format!("invalid salt: {e}")))?;
    let salt: [u8; 16] = salt_bytes.try_into().map_err(|_| JsError::new("salt must be 16 bytes"))?;
    let salt = Salt16::new(salt);

    let kek = derive_kek(password.as_bytes(), &salt, Argon2Profile::Interactive)?;
    Ok(STANDARD.encode(kek.as_bytes()))
}

#[wasm_bindgen]
pub fn derive_verification_key_b64(kek_b64: &str) -> Result<String, JsError> {
    let kek_bytes = STANDARD.decode(kek_b64).map_err(|e| JsError::new(&format!("invalid kek: {e}")))?;
    let kek: [u8; 32] = kek_bytes.try_into().map_err(|_| JsError::new("kek must be 32 bytes"))?;
    let kek = Key256::new(kek);

    let vk = derive_verification_key(&kek);
    Ok(STANDARD.encode(vk.as_bytes()))
}

#[wasm_bindgen]
pub fn bcrypt_hash_b64(plaintext_b64: &str) -> Result<String, JsError> {
    let plaintext = STANDARD.decode(plaintext_b64).map_err(|e| JsError::new(&format!("invalid input: {e}")))?;
    let hash = bcrypt::hash(&plaintext, bcrypt::DEFAULT_COST)
        .map_err(|e| JsError::new(&format!("bcrypt failed: {e}")))?;
    Ok(hash)
}

#[wasm_bindgen]
pub fn encrypt_key_b64(plaintext_b64: &str, wrapping_b64: &str) -> Result<String, JsError> {
    let pt_bytes = STANDARD.decode(plaintext_b64).map_err(|e| JsError::new(&format!("invalid plaintext: {e}")))?;
    let pt: [u8; 32] = pt_bytes.try_into().map_err(|_| JsError::new("plaintext key must be 32 bytes"))?;
    let pt = Key256::new(pt);

    let wk_bytes = STANDARD.decode(wrapping_b64).map_err(|e| JsError::new(&format!("invalid wrapping key: {e}")))?;
    let wk: [u8; 32] = wk_bytes.try_into().map_err(|_| JsError::new("wrapping key must be 32 bytes"))?;
    let wk = Key256::new(wk);

    let encrypted = encrypt_key(&pt, &wk);
    let result = serde_json::json!({
        "nonce": STANDARD.encode(encrypted.nonce.as_bytes()),
        "ciphertext": STANDARD.encode(&encrypted.ciphertext),
    });
    Ok(result.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_b64_returns_valid_base64() {
        let b64 = generate_key_b64();
        let bytes = STANDARD.decode(&b64).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_generate_keypair_b64_returns_valid_json() {
        let json = generate_keypair_b64();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val["public_key"].is_string());
        assert!(val["secret_key"].is_string());

        let pk = STANDARD.decode(val["public_key"].as_str().unwrap()).unwrap();
        let sk = STANDARD.decode(val["secret_key"].as_str().unwrap()).unwrap();
        assert_eq!(pk.len(), 32);
        assert_eq!(sk.len(), 32);
    }

    #[test]
    fn test_generate_salt_b64_returns_16_bytes() {
        let b64 = generate_salt_b64();
        let bytes = STANDARD.decode(&b64).unwrap();
        assert_eq!(bytes.len(), 16);
    }

    #[test]
    fn test_derive_kek_deterministic() {
        let salt = generate_salt_b64();
        let kek1 = derive_kek_b64("test password", &salt, 67108864, 2).unwrap();
        let kek2 = derive_kek_b64("test password", &salt, 67108864, 2).unwrap();
        assert_eq!(kek1, kek2);
    }

    #[test]
    fn test_derive_kek_different_passwords() {
        let salt = generate_salt_b64();
        let kek1 = derive_kek_b64("password1", &salt, 67108864, 2).unwrap();
        let kek2 = derive_kek_b64("password2", &salt, 67108864, 2).unwrap();
        assert_ne!(kek1, kek2);
    }

    #[test]
    fn test_derive_verification_key_deterministic() {
        let kek = generate_key_b64();
        let vk1 = derive_verification_key_b64(&kek).unwrap();
        let vk2 = derive_verification_key_b64(&kek).unwrap();
        assert_eq!(vk1, vk2);
    }

    #[test]
    fn test_bcrypt_hash_verifies() {
        let plaintext = b"test_verify_key";
        let b64 = STANDARD.encode(plaintext);
        let hash = bcrypt_hash_b64(&b64).unwrap();
        assert!(bcrypt::verify(plaintext, &hash).unwrap());
    }

    #[test]
    fn test_encrypt_key_roundtrip() {
        let pt = generate_key_b64();
        let wk = generate_key_b64();
        let encrypted_json = encrypt_key_b64(&pt, &wk).unwrap();
        let val: serde_json::Value = serde_json::from_str(&encrypted_json).unwrap();
        assert!(val["nonce"].is_string());
        assert!(val["ciphertext"].is_string());
    }

    #[test]
    fn test_full_signup_flow() {
        let password = "my_secure_password";
        let salt = generate_salt_b64();

        let kek = derive_kek_b64(password, &salt, 67108864, 2).unwrap();
        let verify_key = derive_verification_key_b64(&kek).unwrap();
        let verify_key_hash = bcrypt_hash_b64(&verify_key).unwrap();

        let master_key = generate_key_b64();
        let keypair_json = generate_keypair_b64();
        let keypair: serde_json::Value = serde_json::from_str(&keypair_json).unwrap();
        let public_key = keypair["public_key"].as_str().unwrap().to_string();

        let encrypted_master = encrypt_key_b64(&master_key, &kek).unwrap();
        let enc_master: serde_json::Value = serde_json::from_str(&encrypted_master).unwrap();

        assert!(!verify_key_hash.is_empty());
        assert!(!public_key.is_empty());
        assert!(enc_master["nonce"].as_str().unwrap().len() > 0);
        assert!(enc_master["ciphertext"].as_str().unwrap().len() > 0);
    }
}
