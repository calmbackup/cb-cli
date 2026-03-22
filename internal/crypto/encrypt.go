package crypto

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"fmt"
	"os"
)

const (
	versionByte1 = 0x01
	versionByte2 = 0x00
	versionSize  = 2
	ivSize       = 12
	tagSize      = 16
	headerSize   = versionSize + ivSize + tagSize // 30 bytes
)

// DeriveKey computes SHA-256 of the key string, returning a 32-byte key.
func DeriveKey(key string) []byte {
	h := sha256.Sum256([]byte(key))
	return h[:]
}

// Encrypt encrypts the file at inputPath and writes the result to outputPath.
// Output format: [VERSION: 2 bytes] [IV: 12 bytes] [TAG: 16 bytes] [CIPHERTEXT]
func Encrypt(inputPath, outputPath string, key []byte) error {
	plaintext, err := os.ReadFile(inputPath)
	if err != nil {
		return fmt.Errorf("reading input file: %w", err)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return fmt.Errorf("creating cipher: %w", err)
	}

	aead, err := cipher.NewGCM(block)
	if err != nil {
		return fmt.Errorf("creating GCM: %w", err)
	}

	iv := make([]byte, ivSize)
	if _, err := rand.Read(iv); err != nil {
		return fmt.Errorf("generating IV: %w", err)
	}

	// Go's Seal returns ciphertext || tag (tag appended at end)
	sealed := aead.Seal(nil, iv, plaintext, nil)
	ctLen := len(sealed) - tagSize
	ciphertext := sealed[:ctLen]
	tag := sealed[ctLen:]

	// Build output: version + IV + tag + ciphertext (PHP-compatible format)
	out := make([]byte, 0, headerSize+len(ciphertext))
	out = append(out, versionByte1, versionByte2)
	out = append(out, iv...)
	out = append(out, tag...)
	out = append(out, ciphertext...)

	if err := os.WriteFile(outputPath, out, 0644); err != nil {
		return fmt.Errorf("writing output file: %w", err)
	}

	return nil
}

// Decrypt decrypts the file at inputPath and writes the result to outputPath.
// Input format: [VERSION: 2 bytes] [IV: 12 bytes] [TAG: 16 bytes] [CIPHERTEXT]
func Decrypt(inputPath, outputPath string, key []byte) error {
	data, err := os.ReadFile(inputPath)
	if err != nil {
		return fmt.Errorf("reading input file: %w", err)
	}

	if len(data) < headerSize {
		return fmt.Errorf("file too small: expected at least %d bytes, got %d", headerSize, len(data))
	}

	// Parse header
	iv := data[versionSize : versionSize+ivSize]
	tag := data[versionSize+ivSize : headerSize]
	ciphertext := data[headerSize:]

	block, err := aes.NewCipher(key)
	if err != nil {
		return fmt.Errorf("creating cipher: %w", err)
	}

	aead, err := cipher.NewGCM(block)
	if err != nil {
		return fmt.Errorf("creating GCM: %w", err)
	}

	// Go's Open expects ciphertext || tag
	sealed := make([]byte, 0, len(ciphertext)+tagSize)
	sealed = append(sealed, ciphertext...)
	sealed = append(sealed, tag...)

	plaintext, err := aead.Open(nil, iv, sealed, nil)
	if err != nil {
		return fmt.Errorf("decryption failed: %w", err)
	}

	if err := os.WriteFile(outputPath, plaintext, 0644); err != nil {
		return fmt.Errorf("writing output file: %w", err)
	}

	return nil
}

// VerifyKey checks if the given key can successfully decrypt the encrypted file.
func VerifyKey(encryptedPath string, key []byte) bool {
	data, err := os.ReadFile(encryptedPath)
	if err != nil {
		return false
	}

	if len(data) < headerSize {
		return false
	}

	iv := data[versionSize : versionSize+ivSize]
	tag := data[versionSize+ivSize : headerSize]
	ciphertext := data[headerSize:]

	block, err := aes.NewCipher(key)
	if err != nil {
		return false
	}

	aead, err := cipher.NewGCM(block)
	if err != nil {
		return false
	}

	sealed := make([]byte, 0, len(ciphertext)+tagSize)
	sealed = append(sealed, ciphertext...)
	sealed = append(sealed, tag...)

	_, err = aead.Open(nil, iv, sealed, nil)
	return err == nil
}
