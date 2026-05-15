use rand_core::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

pub fn generate_keypair() -> (StaticSecret, PublicKey) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret, public)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair_produces_valid_pair() {
        let (secret, public) = generate_keypair();
        let recomputed = PublicKey::from(&secret);
        assert_eq!(public.to_bytes(), recomputed.to_bytes());
    }

    #[test]
    fn test_generate_keypair_produces_different_pairs() {
        let (_, public1) = generate_keypair();
        let (_, public2) = generate_keypair();
        assert_ne!(public1.to_bytes(), public2.to_bytes());
    }
}
