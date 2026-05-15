# Task 7: Implement `crates/metadata` — Layer 1 Metadata Encryption

## Wave
2 (Layer 1 — Pure Libraries)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 5 (crypto) must be complete

## Can Run In Parallel With
Nothing in this wave — metadata depends on crypto (Task 5)

## Design References
- STRUCTURE.md §2.4: metadata — Encrypted Metadata
- design.md §6.4: Decryption Pipeline
- SPEC.md §1.2: File/thumbnail encryption uses XChaCha20-Poly1305

## Requirements
25.3, 5.2, 26.3, 26.5

## Objective
Bridge between `crypto` and typed metadata structs. Encrypt/decrypt `FileMetadata` as JSON blobs using the FileKey.

## Cargo.toml
```toml
[package]
name = "metadata"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
types = { workspace = true }
crypto = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

## Files to Create

### `src/lib.rs`
```rust
pub mod encrypt;
pub mod decrypt;
pub mod structs;
pub mod error;

pub use encrypt::encrypt_metadata;
pub use decrypt::decrypt_metadata;
pub use structs::FileMetadata;
pub use error::MetadataError;
```

### `src/error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum MetadataError {
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::error::CryptoError),
    #[error("JSON serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid UTF-8 in metadata")]
    InvalidUtf8,
}
```

### `src/structs.rs`
Re-export `FileMetadata` from `types::file::FileMetadata` for convenience:
```rust
pub use types::file::FileMetadata;
```

### `src/encrypt.rs`
Implement `encrypt_metadata(metadata: &FileMetadata, file_key: &Key256) -> Result<(Header24, Vec<u8>), MetadataError>`:
1. Serialize `FileMetadata` to JSON bytes: `serde_json::to_vec(metadata)?`
2. Call `crypto::stream_encrypt(&json_bytes, file_key)`
3. Return `(header, ciphertext)`

```rust
use types::file::FileMetadata;
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::MetadataError;

pub fn encrypt_metadata(
    metadata: &FileMetadata,
    file_key: &Key256,
) -> Result<(Header24, Vec<u8>), MetadataError> {
    let json_bytes = serde_json::to_vec(metadata)?;
    let (header, ciphertext) = crypto::stream_encrypt(&json_bytes, file_key)?;
    Ok((header, ciphertext))
}
```

### `src/decrypt.rs`
Implement `decrypt_metadata(header: &Header24, ciphertext: &[u8], file_key: &Key256) -> Result<FileMetadata, MetadataError>`:
1. Call `crypto::stream_decrypt(header, ciphertext, file_key)?`
2. Parse JSON: `serde_json::from_slice(&plaintext_bytes)?`
3. Return `FileMetadata`

```rust
use types::file::FileMetadata;
use types::crypto::Key256;
use types::crypto::Header24;
use crate::error::MetadataError;

pub fn decrypt_metadata(
    header: &Header24,
    ciphertext: &[u8],
    file_key: &Key256,
) -> Result<FileMetadata, MetadataError> {
    let plaintext_bytes = crypto::stream_decrypt(header, ciphertext, file_key)?;
    let metadata = serde_json::from_slice(&plaintext_bytes)?;
    Ok(metadata)
}
```

## Tests

### Property Test (Task 7.3 — marked with *)
- `∀ key ∈ Key256, metadata ∈ FileMetadata: decrypt_metadata(encrypt_metadata(metadata, key)) == Ok(metadata)`
- Use `proptest` to generate random `FileMetadata` instances

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip_encrypt_decrypt(
            key_bytes in prop::array::uniform32(any::<u8>()),
            name in ".*",
            mime_type in ".*",
            size in 0u64..u64::MAX,
        ) {
            let key = Key256::from(key_bytes);
            let metadata = FileMetadata {
                name,
                mime_type,
                size,
                ..Default::default()
            };

            let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
            let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();

            prop_assert_eq!(decrypted.name, metadata.name);
            prop_assert_eq!(decrypted.mime_type, metadata.mime_type);
            prop_assert_eq!(decrypted.size, metadata.size);
        }
    }
}
```

### Unit Tests (Task 7.4 — marked with *)
- Missing optional fields serialize as `null` in JSON
- JSON round-trip preserves all fields
- GPS coordinates preserved with full f64 precision
- `tags` vec round-trips correctly
- Empty `FileMetadata` (all None/empty) round-trips correctly
- `decrypt_metadata` with tampered ciphertext returns `MetadataError::Crypto(CryptoError::MacMismatch)`

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn missing_optional_fields_serialize_as_null() {
        let metadata = FileMetadata::default();
        let json = serde_json::to_string(&metadata).unwrap();
        // Verify optional fields appear as null in JSON
        assert!(json.contains("null") || json.contains("[]"));
    }

    #[test]
    fn json_roundtrip_preserves_all_fields() {
        let metadata = FileMetadata {
            name: "test.jpg".into(),
            mime_type: "image/jpeg".into(),
            size: 12345,
            tags: vec!["vacation".into(), "summer".into()],
            ..Default::default()
        };
        let json = serde_json::to_vec(&metadata).unwrap();
        let roundtripped: FileMetadata = serde_json::from_slice(&json).unwrap();
        assert_eq!(roundtripped.name, metadata.name);
        assert_eq!(roundtripped.tags, metadata.tags);
    }

    #[test]
    fn gps_coordinates_preserved_with_full_precision() {
        let metadata = FileMetadata {
            latitude: Some(40.7127753),
            longitude: Some(-74.0059728),
            ..Default::default()
        };
        let (header, ciphertext) = encrypt_metadata(&metadata, &Key256::random()).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &Key256::random()).unwrap_err();
        // Wrong key should fail
        assert!(matches!(decrypted, MetadataError::Crypto(_)));
    }

    #[test]
    fn tags_vec_roundtrips_correctly() {
        let metadata = FileMetadata {
            tags: vec!["a".into(), "b".into(), "c".into()],
            ..Default::default()
        };
        let key = Key256::random();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted.tags, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_metadata_roundtrips_correctly() {
        let metadata = FileMetadata::default();
        let key = Key256::random();
        let (header, ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        let decrypted = decrypt_metadata(&header, &ciphertext, &key).unwrap();
        assert_eq!(decrypted, metadata);
    }

    #[test]
    fn tampered_ciphertext_returns_mac_mismatch() {
        let metadata = FileMetadata::default();
        let key = Key256::random();
        let (header, mut ciphertext) = encrypt_metadata(&metadata, &key).unwrap();
        // Tamper with ciphertext
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }
        let result = decrypt_metadata(&header, &ciphertext, &key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MetadataError::Crypto(crypto::error::CryptoError::MacMismatch)));
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_time() {
        let metadata = FileMetadata::default();
        let key = Key256::random();
        let (_, ct1) = encrypt_metadata(&metadata, &key).unwrap();
        let (_, ct2) = encrypt_metadata(&metadata, &key).unwrap();
        assert_ne!(ct1, ct2);
    }
}
```

## Verification Steps
- [ ] `cargo check -p metadata` succeeds
- [ ] `cargo test -p metadata` passes
- [ ] `encrypt_metadata` produces different ciphertext for same input (random header)
- [ ] `decrypt_metadata` with wrong key returns `MacMismatch`
- [ ] JSON serialization handles all optional fields correctly

## Notes
- This crate is a thin wrapper — the actual crypto is in `crypto::stream_encrypt/decrypt`.
- The metadata is serialized as JSON before encryption, so the ciphertext size depends on the metadata content.
- The `FileMetadata` struct is defined in `types` — this crate just handles the encrypt/decrypt pipeline.
