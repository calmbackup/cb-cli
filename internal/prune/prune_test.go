package prune

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func createTestFile(t *testing.T, dir, name string, age time.Duration) {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte("data"), 0644); err != nil {
		t.Fatalf("failed to create file %s: %v", name, err)
	}

	modTime := time.Now().Add(-age)
	if err := os.Chtimes(path, modTime, modTime); err != nil {
		t.Fatalf("failed to set mtime for %s: %v", name, err)
	}
}

func TestPruneOldAndConfirmed(t *testing.T) {
	dir := t.TempDir()
	createTestFile(t, dir, "old-confirmed.tar.gz.enc", 10*24*time.Hour)
	createTestFile(t, dir, "old-confirmed2.tar.gz.enc", 15*24*time.Hour)

	confirmed := []string{"old-confirmed.tar.gz.enc", "old-confirmed2.tar.gz.enc"}
	count, err := Prune(dir, 7, confirmed)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if count != 2 {
		t.Errorf("expected 2 deleted, got %d", count)
	}

	for _, name := range confirmed {
		if _, err := os.Stat(filepath.Join(dir, name)); !os.IsNotExist(err) {
			t.Errorf("expected %s to be deleted", name)
		}
	}
}

func TestPruneOldButNotConfirmed(t *testing.T) {
	dir := t.TempDir()
	createTestFile(t, dir, "old-unconfirmed.tar.gz.enc", 10*24*time.Hour)

	count, err := Prune(dir, 7, []string{})

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if count != 0 {
		t.Errorf("expected 0 deleted, got %d", count)
	}

	if _, err := os.Stat(filepath.Join(dir, "old-unconfirmed.tar.gz.enc")); os.IsNotExist(err) {
		t.Error("expected file to still exist")
	}
}

func TestPruneNewFilesKept(t *testing.T) {
	dir := t.TempDir()
	createTestFile(t, dir, "new-confirmed.tar.gz.enc", 2*24*time.Hour)

	count, err := Prune(dir, 7, []string{"new-confirmed.tar.gz.enc"})

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if count != 0 {
		t.Errorf("expected 0 deleted, got %d", count)
	}

	if _, err := os.Stat(filepath.Join(dir, "new-confirmed.tar.gz.enc")); os.IsNotExist(err) {
		t.Error("expected file to still exist")
	}
}

func TestPruneEmptyDirectory(t *testing.T) {
	dir := t.TempDir()

	count, err := Prune(dir, 7, []string{})

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if count != 0 {
		t.Errorf("expected 0 deleted, got %d", count)
	}
}

func TestPruneNonExistentDirectory(t *testing.T) {
	_, err := Prune("/nonexistent/path/that/does/not/exist", 7, []string{})

	if err == nil {
		t.Fatal("expected error for non-existent directory")
	}
}
