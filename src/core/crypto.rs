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
