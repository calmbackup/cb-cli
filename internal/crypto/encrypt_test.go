package crypto

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

func TestDeriveKey_Length(t *testing.T) {
	key := DeriveKey("test-encryption-key")
	if len(key) != 32 {
		t.Fatalf("expected 32-byte key, got %d bytes", len(key))
	}
}

func TestDeriveKey_Deterministic(t *testing.T) {
	k1 := DeriveKey("my-secret-key")
	k2 := DeriveKey("my-secret-key")
	if !bytesEqual(k1, k2) {
		t.Fatal("DeriveKey should be deterministic")
	}
}

func TestDeriveKey_MatchesSHA256(t *testing.T) {
	input := "test-key-123"
	expected := sha256.Sum256([]byte(input))
	got := DeriveKey(input)
	if !bytesEqual(got, expected[:]) {
		t.Errorf("DeriveKey does not match SHA-256\nexpected: %x\ngot:      %x", expected, got)
	}
}

func TestDeriveKey_KnownVector(t *testing.T) {
	// SHA-256("hello") is a well-known value
	key := DeriveKey("hello")
	expectedHex := "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
	gotHex := hex.EncodeToString(key)
	if gotHex != expectedHex {
		t.Errorf("DeriveKey('hello') mismatch\nexpected: %s\ngot:      %s", expectedHex, gotHex)
	}
}

func TestEncrypt_FileFormat(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "plain.txt")
	outputPath := filepath.Join(dir, "encrypted.bin")

	plaintext := []byte("Hello, World! This is a test.")
	if err := os.WriteFile(inputPath, plaintext, 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("test-key")
	if err := Encrypt(inputPath, outputPath, key); err != nil {
		t.Fatalf("Encrypt failed: %v", err)
	}

	data, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatal(err)
	}

	// Minimum size: 2 (version) + 12 (IV) + 16 (tag) + at least 1 byte ciphertext
	if len(data) < 31 {
		t.Fatalf("encrypted file too small: %d bytes", len(data))
	}

	// Check version bytes
	if data[0] != 0x01 || data[1] != 0x00 {
		t.Errorf("expected version bytes 0x01 0x00, got 0x%02x 0x%02x", data[0], data[1])
	}

	// IV is bytes 2..14 (12 bytes)
	iv := data[2:14]
	if len(iv) != 12 {
		t.Errorf("expected 12-byte IV, got %d", len(iv))
	}

	// Tag is bytes 14..30 (16 bytes)
	tag := data[14:30]
	if len(tag) != 16 {
		t.Errorf("expected 16-byte tag, got %d", len(tag))
	}

	// Ciphertext follows
	ciphertext := data[30:]
	if len(ciphertext) != len(plaintext) {
		t.Errorf("expected ciphertext length %d (same as plaintext for GCM), got %d", len(plaintext), len(ciphertext))
	}
}

func TestEncryptDecrypt_RoundTrip(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "plain.txt")
	encPath := filepath.Join(dir, "encrypted.bin")
	decPath := filepath.Join(dir, "decrypted.txt")

	plaintext := []byte("Round trip test data with special chars: éàü 日本語")
	if err := os.WriteFile(inputPath, plaintext, 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("round-trip-key")
	if err := Encrypt(inputPath, encPath, key); err != nil {
		t.Fatalf("Encrypt failed: %v", err)
	}
	if err := Decrypt(encPath, decPath, key); err != nil {
		t.Fatalf("Decrypt failed: %v", err)
	}

	result, err := os.ReadFile(decPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytesEqual(result, plaintext) {
		t.Errorf("round trip mismatch\nexpected: %q\ngot:      %q", plaintext, result)
	}
}

func TestDecrypt_WrongKey(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "plain.txt")
	encPath := filepath.Join(dir, "encrypted.bin")
	decPath := filepath.Join(dir, "decrypted.txt")

	if err := os.WriteFile(inputPath, []byte("secret data"), 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("correct-key")
	if err := Encrypt(inputPath, encPath, key); err != nil {
		t.Fatal(err)
	}

	wrongKey := DeriveKey("wrong-key")
	err := Decrypt(encPath, decPath, wrongKey)
	if err == nil {
		t.Fatal("expected error when decrypting with wrong key")
	}
}

func TestVerifyKey_Correct(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "plain.txt")
	encPath := filepath.Join(dir, "encrypted.bin")

	if err := os.WriteFile(inputPath, []byte("verify test"), 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("verify-key")
	if err := Encrypt(inputPath, encPath, key); err != nil {
		t.Fatal(err)
	}

	if !VerifyKey(encPath, key) {
		t.Error("VerifyKey should return true for correct key")
	}
}

func TestVerifyKey_Wrong(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "plain.txt")
	encPath := filepath.Join(dir, "encrypted.bin")

	if err := os.WriteFile(inputPath, []byte("verify test"), 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("verify-key")
	if err := Encrypt(inputPath, encPath, key); err != nil {
		t.Fatal(err)
	}

	wrongKey := DeriveKey("other-key")
	if VerifyKey(encPath, wrongKey) {
		t.Error("VerifyKey should return false for wrong key")
	}
}

func TestEncrypt_EmptyFile(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join(dir, "empty.txt")
	encPath := filepath.Join(dir, "encrypted.bin")
	decPath := filepath.Join(dir, "decrypted.txt")

	if err := os.WriteFile(inputPath, []byte{}, 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("empty-key")
	if err := Encrypt(inputPath, encPath, key); err != nil {
		t.Fatalf("Encrypt empty file failed: %v", err)
	}
	if err := Decrypt(encPath, decPath, key); err != nil {
		t.Fatalf("Decrypt empty file failed: %v", err)
	}

	result, err := os.ReadFile(decPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(result) != 0 {
		t.Errorf("expected empty result, got %d bytes", len(result))
	}
}

func TestDecrypt_TruncatedFile(t *testing.T) {
	dir := t.TempDir()
	encPath := filepath.Join(dir, "truncated.bin")
	decPath := filepath.Join(dir, "decrypted.txt")

	// Write a file too short to contain the header
	if err := os.WriteFile(encPath, []byte{0x01, 0x00, 0x01}, 0644); err != nil {
		t.Fatal(err)
	}

	key := DeriveKey("any-key")
	err := Decrypt(encPath, decPath, key)
	if err == nil {
		t.Fatal("expected error for truncated file")
	}
}

func TestEncryptDecrypt_KnownTestVector(t *testing.T) {
	// Test with a fixed IV to verify deterministic output.
	// We manually construct encrypted data using Go's crypto primitives
	// in the PHP-compatible format and then verify Decrypt can handle it.
	keyStr := "known-test-key"
	key := DeriveKey(keyStr)
	plaintext := []byte("Known plaintext for testing")

	// Encrypt manually with a known IV
	fixedIV := []byte{0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b}

	block, err := aes.NewCipher(key)
	if err != nil {
		t.Fatal(err)
	}
	aead, err := cipher.NewGCM(block)
	if err != nil {
		t.Fatal(err)
	}

	// Go Seal returns ciphertext || tag
	sealed := aead.Seal(nil, fixedIV, plaintext, nil)
	ctLen := len(sealed) - 16
	ciphertext := sealed[:ctLen]
	tag := sealed[ctLen:]

	// Build PHP-compatible format: version + IV + tag + ciphertext
	var formatted []byte
	formatted = append(formatted, 0x01, 0x00)
	formatted = append(formatted, fixedIV...)
	formatted = append(formatted, tag...)
	formatted = append(formatted, ciphertext...)

	// Write to file and decrypt
	dir := t.TempDir()
	encPath := filepath.Join(dir, "known.bin")
	decPath := filepath.Join(dir, "decrypted.txt")

	if err := os.WriteFile(encPath, formatted, 0644); err != nil {
		t.Fatal(err)
	}

	if err := Decrypt(encPath, decPath, key); err != nil {
		t.Fatalf("Decrypt known vector failed: %v", err)
	}

	result, err := os.ReadFile(decPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(result) != string(plaintext) {
		t.Errorf("known vector mismatch\nexpected: %q\ngot:      %q", plaintext, result)
	}
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
