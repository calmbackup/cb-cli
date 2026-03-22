package upload

import (
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestUpload(t *testing.T) {
	tmpDir := t.TempDir()
	filePath := filepath.Join(tmpDir, "backup.tar.gz.enc")
	content := []byte("test backup content here")
	if err := os.WriteFile(filePath, content, 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPut {
			t.Errorf("expected PUT, got %s", r.Method)
		}

		if r.Header.Get("Content-Type") != "application/octet-stream" {
			t.Errorf("unexpected content-type: %s", r.Header.Get("Content-Type"))
		}

		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Fatalf("failed to read body: %v", err)
		}

		if string(body) != string(content) {
			t.Errorf("unexpected body: %s", string(body))
		}

		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	err := Upload(filePath, server.URL)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestUploadServerError4xx(t *testing.T) {
	tmpDir := t.TempDir()
	filePath := filepath.Join(tmpDir, "backup.tar.gz.enc")
	if err := os.WriteFile(filePath, []byte("data"), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusForbidden)
	}))
	defer server.Close()

	err := Upload(filePath, server.URL)
	if err == nil {
		t.Fatal("expected error for 403 response")
	}
}

func TestUploadServerError5xx(t *testing.T) {
	tmpDir := t.TempDir()
	filePath := filepath.Join(tmpDir, "backup.tar.gz.enc")
	if err := os.WriteFile(filePath, []byte("data"), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer server.Close()

	err := Upload(filePath, server.URL)
	if err == nil {
		t.Fatal("expected error for 500 response")
	}
}

func TestDownload(t *testing.T) {
	content := []byte("downloaded backup content")
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			t.Errorf("expected GET, got %s", r.Method)
		}

		w.WriteHeader(http.StatusOK)
		w.Write(content)
	}))
	defer server.Close()

	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.tar.gz.enc")

	err := Download(server.URL, outputPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	got, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("failed to read output: %v", err)
	}

	if string(got) != string(content) {
		t.Errorf("unexpected content: %s", string(got))
	}
}

func TestDownloadServerError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusForbidden)
	}))
	defer server.Close()

	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.tar.gz.enc")

	err := Download(server.URL, outputPath)
	if err == nil {
		t.Fatal("expected error for 403 response")
	}
}

func TestDownloadCleansUpPartialFile(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusForbidden)
	}))
	defer server.Close()

	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.tar.gz.enc")

	_ = Download(server.URL, outputPath)

	if _, err := os.Stat(outputPath); !os.IsNotExist(err) {
		t.Error("expected partial file to be cleaned up")
	}
}
