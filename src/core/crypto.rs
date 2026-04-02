use std::path::Path;
use crate::core::types::{AppError, Result};

use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use sha2::{Sha256, Digest};
use rand::RngCore;

/// File format (Go-compatible):
/// [VERSION: 2 bytes (0x01, 0x00)]
/// [IV: 12 bytes]
/// [TAG: 16 bytes]
/// [CIPHERTEXT: remaining bytes]
const VERSION: [u8; 2] = [0x01, 0x00];
const IV_SIZE: usize = 12;
const TAG_SIZE: usize = 16;
const HEADER_SIZE: usize = 2 + IV_SIZE + TAG_SIZE; // 30 bytes

/// Derive a 32-byte AES-256 key from the config encryption key string.
/// Uses SHA-256, matching the Go implementation.
pub fn derive_key(key_string: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(key_string.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt a file using AES-256-GCM.
/// Writes Go-compatible format: VERSION + IV + TAG + CIPHERTEXT.
pub fn encrypt(input_path: &Path, output_path: &Path, key: &[u8; 32]) -> Result<()> {
    let plaintext = std::fs::read(input_path).map_err(|e| {
        AppError::Crypto(format!("Failed to read input file: {}", e))
    })?;

    // Generate random 12-byte IV
    let mut iv = [0u8; IV_SIZE];
    rand::thread_rng().fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    // Encrypt with AES-256-GCM
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

    // aes-gcm crate returns ciphertext || tag (tag is last 16 bytes)
    let encrypted = cipher.encrypt(nonce, plaintext.as_ref())
        .map_err(|e| AppError::Crypto(format!("Encryption failed: {}", e)))?;

    // Split into ciphertext and tag for Go-compatible format
    let ct_len = encrypted.len() - TAG_SIZE;
    let ciphertext = &encrypted[..ct_len];
    let tag = &encrypted[ct_len..];

    // Write: VERSION + IV + TAG + CIPHERTEXT
    let mut output = Vec::with_capacity(HEADER_SIZE + ciphertext.len());
    output.extend_from_slice(&VERSION);
    output.extend_from_slice(&iv);
    output.extend_from_slice(tag);
    output.extend_from_slice(ciphertext);

    std::fs::write(output_path, &output).map_err(|e| {
        AppError::Crypto(format!("Failed to write encrypted file: {}", e))
    })?;

    Ok(())
}

/// Decrypt a file encrypted with AES-256-GCM.
/// Reads Go-compatible format: VERSION + IV + TAG + CIPHERTEXT.
pub fn decrypt(input_path: &Path, output_path: &Path, key: &[u8; 32]) -> Result<()> {
    let data = std::fs::read(input_path).map_err(|e| {
        AppError::Crypto(format!("Failed to read encrypted file: {}", e))
    })?;

    let plaintext = decrypt_bytes(&data, key)?;

    std::fs::write(output_path, &plaintext).map_err(|e| {
        AppError::Crypto(format!("Failed to write decrypted file: {}", e))
    })?;

    Ok(())
}

/// Internal: decrypt raw bytes in Go-compatible format.
fn decrypt_bytes(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() < HEADER_SIZE {
        return Err(AppError::Crypto("File too small to be a valid encrypted file".to_string()));
    }

    // Verify version bytes
    if data[0] != VERSION[0] || data[1] != VERSION[1] {
        return Err(AppError::Crypto(format!(
            "Unsupported file version: {:02x}{:02x}",
            data[0], data[1]
        )));
    }

    // Parse header
    let iv = &data[2..2 + IV_SIZE];
    let tag = &data[2 + IV_SIZE..2 + IV_SIZE + TAG_SIZE];
    let ciphertext = &data[HEADER_SIZE..];

    let nonce = Nonce::from_slice(iv);

    // Reassemble into ciphertext || tag format expected by aes-gcm crate
    let mut combined = Vec::with_capacity(ciphertext.len() + TAG_SIZE);
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(tag);

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Crypto(format!("Failed to create cipher: {}", e)))?;

    let plaintext = cipher.decrypt(nonce, combined.as_ref())
        .map_err(|e| AppError::Crypto(format!("Decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Verify that a key can decrypt an encrypted file (test decryption without writing output).
pub fn verify_key(encrypted_path: &Path, key: &[u8; 32]) -> Result<bool> {
    let data = std::fs::read(encrypted_path).map_err(|e| {
        AppError::Crypto(format!("Failed to read encrypted file: {}", e))
    })?;

    match decrypt_bytes(&data, key) {
        Ok(_) => Ok(true),
        Err(AppError::Crypto(msg)) if msg.contains("Decryption failed") => Ok(false),
        Err(e) => Err(e),
    }
}

/// Compute SHA-256 checksum of a file, returned as hex string.
pub fn checksum(file_path: &Path) -> Result<String> {
    let data = std::fs::read(file_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: create a temp file with the given contents, return its path.
    /// The returned TempFile keeps the file alive; drop it to delete.
    struct TempFile {
        path: std::path::PathBuf,
    }

    impl TempFile {
        fn new(name: &str, contents: &[u8]) -> Self {
            let dir = std::env::temp_dir().join("calmbackup_tests");
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(name);
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(contents).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        /// Create a TempFile with no content on disk yet (path only).
        fn empty_path(name: &str) -> Self {
            let dir = std::env::temp_dir().join("calmbackup_tests");
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(name);
            Self { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    // ---------------------------------------------------------------
    // 1. derive_key
    // ---------------------------------------------------------------

    #[test]
    fn derive_key_known_vector() {
        // SHA-256("test-key") pre-computed
        let key = derive_key("test-key");
        let hex_key = hex::encode(key);
        // Compute expected via sha2 directly for cross-check
        let mut hasher = Sha256::new();
        hasher.update(b"test-key");
        let expected = hex::encode(hasher.finalize());
        assert_eq!(hex_key, expected);
    }

    #[test]
    fn derive_key_empty_string() {
        let key = derive_key("");
        assert_eq!(key.len(), 32);
        // SHA-256 of empty string is a well-known value
        let hex_key = hex::encode(key);
        assert_eq!(
            hex_key,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn derive_key_deterministic() {
        let k1 = derive_key("my-secret");
        let k2 = derive_key("my-secret");
        assert_eq!(k1, k2);
    }

    // ---------------------------------------------------------------
    // 2. encrypt/decrypt roundtrip
    // ---------------------------------------------------------------

    #[test]
    fn roundtrip_basic() {
        let plaintext = b"Hello, Calm Backup!";
        let input = TempFile::new("rt_basic_in.txt", plaintext);
        let encrypted = TempFile::empty_path("rt_basic_enc.bin");
        let decrypted = TempFile::empty_path("rt_basic_dec.txt");

        let key = derive_key("roundtrip-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();
        decrypt(encrypted.path(), decrypted.path(), &key).unwrap();

        let result = std::fs::read(decrypted.path()).unwrap();
        assert_eq!(result, plaintext);
    }

    #[test]
    fn roundtrip_empty_file() {
        let input = TempFile::new("rt_empty_in.txt", b"");
        let encrypted = TempFile::empty_path("rt_empty_enc.bin");
        let decrypted = TempFile::empty_path("rt_empty_dec.txt");

        let key = derive_key("empty-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();
        decrypt(encrypted.path(), decrypted.path(), &key).unwrap();

        let result = std::fs::read(decrypted.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn roundtrip_large_content() {
        // 1 MB of pseudo-random data
        let mut data = vec![0u8; 1_000_000];
        rand::thread_rng().fill_bytes(&mut data);

        let input = TempFile::new("rt_large_in.bin", &data);
        let encrypted = TempFile::empty_path("rt_large_enc.bin");
        let decrypted = TempFile::empty_path("rt_large_dec.bin");

        let key = derive_key("large-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();
        decrypt(encrypted.path(), decrypted.path(), &key).unwrap();

        let result = std::fs::read(decrypted.path()).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn roundtrip_binary_content() {
        // All byte values 0x00..0xFF repeated
        let data: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
        let input = TempFile::new("rt_binary_in.bin", &data);
        let encrypted = TempFile::empty_path("rt_binary_enc.bin");
        let decrypted = TempFile::empty_path("rt_binary_dec.bin");

        let key = derive_key("binary-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();
        decrypt(encrypted.path(), decrypted.path(), &key).unwrap();

        let result = std::fs::read(decrypted.path()).unwrap();
        assert_eq!(result, data);
    }

    // ---------------------------------------------------------------
    // 3. Go-compatible file format verification
    // ---------------------------------------------------------------

    #[test]
    fn encrypted_file_format() {
        let plaintext = b"format check payload";
        let input = TempFile::new("fmt_in.txt", plaintext);
        let encrypted = TempFile::empty_path("fmt_enc.bin");

        let key = derive_key("format-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();

        let data = std::fs::read(encrypted.path()).unwrap();

        // Version bytes
        assert_eq!(data[0], 0x01, "version byte 0");
        assert_eq!(data[1], 0x00, "version byte 1");

        // IV: 12 bytes starting at offset 2
        let iv = &data[2..14];
        assert_eq!(iv.len(), 12);
        // IV should not be all zeros (astronomically unlikely for random)
        assert!(iv.iter().any(|&b| b != 0), "IV should not be all zeros");

        // TAG: 16 bytes starting at offset 14
        let tag = &data[14..30];
        assert_eq!(tag.len(), 16);

        // Ciphertext: remaining bytes
        let ciphertext = &data[30..];
        // For GCM, ciphertext length equals plaintext length
        assert_eq!(ciphertext.len(), plaintext.len());

        // Total file size
        assert_eq!(data.len(), HEADER_SIZE + plaintext.len());
    }

    // ---------------------------------------------------------------
    // 4. decrypt with wrong key
    // ---------------------------------------------------------------

    #[test]
    fn decrypt_wrong_key_fails() {
        let plaintext = b"secret data";
        let input = TempFile::new("wk_in.txt", plaintext);
        let encrypted = TempFile::empty_path("wk_enc.bin");
        let decrypted = TempFile::empty_path("wk_dec.txt");

        let key_a = derive_key("key-a");
        let key_b = derive_key("key-b");

        encrypt(input.path(), encrypted.path(), &key_a).unwrap();
        let result = decrypt(encrypted.path(), decrypted.path(), &key_b);
        assert!(result.is_err(), "decrypting with wrong key should fail");
    }

    // ---------------------------------------------------------------
    // 5. decrypt malformed data
    // ---------------------------------------------------------------

    #[test]
    fn decrypt_file_too_small() {
        let small = TempFile::new("small.bin", &[0u8; 10]);
        let out = TempFile::empty_path("small_out.bin");
        let key = derive_key("k");

        let result = decrypt(small.path(), out.path(), &key);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("too small"), "error should mention too small: {msg}");
    }

    #[test]
    fn decrypt_wrong_version() {
        // Build a file with wrong version bytes but correct size
        let mut data = vec![0u8; 50];
        data[0] = 0xFF;
        data[1] = 0xFF;
        let bad = TempFile::new("bad_version.bin", &data);
        let out = TempFile::empty_path("bad_version_out.bin");
        let key = derive_key("k");

        let result = decrypt(bad.path(), out.path(), &key);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("version"), "error should mention version: {msg}");
    }

    #[test]
    fn decrypt_corrupted_ciphertext() {
        let plaintext = b"will be corrupted";
        let input = TempFile::new("corrupt_in.txt", plaintext);
        let encrypted = TempFile::empty_path("corrupt_enc.bin");
        let decrypted = TempFile::empty_path("corrupt_dec.txt");

        let key = derive_key("corrupt-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();

        // Flip a byte in the ciphertext region
        let mut data = std::fs::read(encrypted.path()).unwrap();
        let last = data.len() - 1;
        data[last] ^= 0xFF;
        std::fs::write(encrypted.path(), &data).unwrap();

        let result = decrypt(encrypted.path(), decrypted.path(), &key);
        assert!(result.is_err(), "corrupted ciphertext should fail decryption");
    }

    // ---------------------------------------------------------------
    // 6. verify_key
    // ---------------------------------------------------------------

    #[test]
    fn verify_key_correct() {
        let input = TempFile::new("vk_in.txt", b"verify me");
        let encrypted = TempFile::empty_path("vk_enc.bin");

        let key = derive_key("verify-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();

        assert_eq!(verify_key(encrypted.path(), &key).unwrap(), true);
    }

    #[test]
    fn verify_key_wrong() {
        let input = TempFile::new("vk_wrong_in.txt", b"verify me");
        let encrypted = TempFile::empty_path("vk_wrong_enc.bin");

        let key = derive_key("verify-key");
        let wrong = derive_key("wrong-key");
        encrypt(input.path(), encrypted.path(), &key).unwrap();

        assert_eq!(verify_key(encrypted.path(), &wrong).unwrap(), false);
    }

    #[test]
    fn verify_key_malformed_file() {
        let bad = TempFile::new("vk_bad.bin", &[0u8; 5]);
        let key = derive_key("k");

        let result = verify_key(bad.path(), &key);
        assert!(result.is_err(), "malformed file should return Err, not Ok(false)");
    }

    // ---------------------------------------------------------------
    // 7. checksum
    // ---------------------------------------------------------------

    #[test]
    fn checksum_known_content() {
        // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let f = TempFile::new("cksum.txt", b"hello");
        let sum = checksum(f.path()).unwrap();
        assert_eq!(
            sum,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn checksum_empty_file() {
        let f = TempFile::new("cksum_empty.txt", b"");
        let sum = checksum(f.path()).unwrap();
        assert_eq!(
            sum,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // ---------------------------------------------------------------
    // 8. Cross-compatibility: manually construct a Go-format file
    // ---------------------------------------------------------------

    #[test]
    fn decrypt_go_compatible_constructed_file() {
        // Simulate what Go would produce:
        // 1. Pick a known key, IV, plaintext
        // 2. Encrypt with AES-256-GCM using aes-gcm crate
        // 3. Arrange as VERSION + IV + TAG + CIPHERTEXT
        // 4. Write to file
        // 5. Call decrypt() and verify plaintext

        let key_bytes = derive_key("go-compat-key");
        let iv: [u8; 12] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
                             0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C];
        let plaintext = b"Go says hello to Rust";

        // Encrypt with AES-256-GCM directly
        let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();
        let nonce = Nonce::from_slice(&iv);
        let encrypted = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

        // aes-gcm returns ciphertext || tag
        let ct_len = encrypted.len() - TAG_SIZE;
        let ciphertext = &encrypted[..ct_len];
        let tag = &encrypted[ct_len..];

        // Build Go-compatible file: VERSION + IV + TAG + CIPHERTEXT
        let mut file_data = Vec::new();
        file_data.extend_from_slice(&[0x01, 0x00]); // version
        file_data.extend_from_slice(&iv);
        file_data.extend_from_slice(tag);
        file_data.extend_from_slice(ciphertext);

        let enc_file = TempFile::new("go_compat_enc.bin", &file_data);
        let dec_file = TempFile::empty_path("go_compat_dec.txt");

        decrypt(enc_file.path(), dec_file.path(), &key_bytes).unwrap();

        let result = std::fs::read(dec_file.path()).unwrap();
        assert_eq!(result, plaintext, "decrypted output must match original plaintext");
    }
}
