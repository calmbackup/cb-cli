use std::path::Path;
use crate::core::types::Result;

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
    todo!("SHA-256 hash of key_string")
}

/// Encrypt a file using AES-256-GCM.
/// Writes Go-compatible format: VERSION + IV + TAG + CIPHERTEXT.
pub fn encrypt(input_path: &Path, output_path: &Path, key: &[u8; 32]) -> Result<()> {
    todo!("Read input, generate random IV, encrypt with AES-256-GCM, write formatted output")
}

/// Decrypt a file encrypted with AES-256-GCM.
/// Reads Go-compatible format: VERSION + IV + TAG + CIPHERTEXT.
pub fn decrypt(input_path: &Path, output_path: &Path, key: &[u8; 32]) -> Result<()> {
    todo!("Read formatted input, parse header, decrypt with AES-256-GCM, write output")
}

/// Verify that a key can decrypt an encrypted file (test decryption without writing output).
pub fn verify_key(encrypted_path: &Path, key: &[u8; 32]) -> Result<bool> {
    todo!("Try to decrypt, return true if successful")
}

/// Compute SHA-256 checksum of a file, returned as hex string.
pub fn checksum(file_path: &Path) -> Result<String> {
    todo!("Read file, compute SHA-256, return hex")
}
