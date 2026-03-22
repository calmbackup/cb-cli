package api

import (
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestRequestUploadURL(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("expected POST, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/upload-url" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		if r.Header.Get("Authorization") != "Bearer test-key" {
			t.Errorf("unexpected auth header: %s", r.Header.Get("Authorization"))
		}

		if r.Header.Get("Content-Type") != "application/json" {
			t.Errorf("unexpected content-type: %s", r.Header.Get("Content-Type"))
		}

		if r.Header.Get("Accept") != "application/json" {
			t.Errorf("unexpected accept: %s", r.Header.Get("Accept"))
		}

		if r.Header.Get("X-Backup-Version") != "1.0.0" {
			t.Errorf("unexpected version header: %s", r.Header.Get("X-Backup-Version"))
		}

		var body map[string]interface{}
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			t.Fatalf("failed to decode body: %v", err)
		}

		if body["filename"] != "backup.tar.gz.enc" {
			t.Errorf("unexpected filename: %v", body["filename"])
		}

		if body["size"] != float64(1024) {
			t.Errorf("unexpected size: %v", body["size"])
		}

		if body["checksum"] != "abc123" {
			t.Errorf("unexpected checksum: %v", body["checksum"])
		}

		if body["db_driver"] != "mysql" {
			t.Errorf("unexpected db_driver: %v", body["db_driver"])
		}

		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(UploadURLResponse{
			BackupID:  "backup-1",
			UploadURL: "https://s3.example.com/upload",
			ExpiresIn: 3600,
		})
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	resp, err := client.RequestUploadURL("backup.tar.gz.enc", 1024, "abc123", "mysql")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp.BackupID != "backup-1" {
		t.Errorf("unexpected backup_id: %s", resp.BackupID)
	}

	if resp.UploadURL != "https://s3.example.com/upload" {
		t.Errorf("unexpected upload_url: %s", resp.UploadURL)
	}

	if resp.ExpiresIn != 3600 {
		t.Errorf("unexpected expires_in: %d", resp.ExpiresIn)
	}
}

func TestConfirmBackup(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("expected POST, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/backups/backup-1/confirm" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		var body map[string]interface{}
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			t.Fatalf("failed to decode body: %v", err)
		}

		if body["size"] != float64(2048) {
			t.Errorf("unexpected size: %v", body["size"])
		}

		if body["checksum"] != "def456" {
			t.Errorf("unexpected checksum: %v", body["checksum"])
		}

		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	err := client.ConfirmBackup("backup-1", 2048, "def456")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestListBackups(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			t.Errorf("expected GET, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/backups" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		if r.URL.Query().Get("page") != "2" {
			t.Errorf("unexpected page: %s", r.URL.Query().Get("page"))
		}

		if r.URL.Query().Get("per_page") != "10" {
			t.Errorf("unexpected per_page: %s", r.URL.Query().Get("per_page"))
		}

		resp := ListBackupsResponse{
			Data: []Backup{
				{ID: "b1", Filename: "backup1.tar.gz.enc", Size: 100},
				{ID: "b2", Filename: "backup2.tar.gz.enc", Size: 200},
			},
		}
		resp.Meta.CurrentPage = 2
		resp.Meta.LastPage = 5
		resp.Meta.Total = 50

		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(resp)
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	resp, err := client.ListBackups(2, 10)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.Data) != 2 {
		t.Fatalf("expected 2 backups, got %d", len(resp.Data))
	}

	if resp.Data[0].ID != "b1" {
		t.Errorf("unexpected first backup id: %s", resp.Data[0].ID)
	}

	if resp.Meta.Total != 50 {
		t.Errorf("unexpected total: %d", resp.Meta.Total)
	}
}

func TestGetBackup(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			t.Errorf("expected GET, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/backups/backup-1" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		json.NewEncoder(w).Encode(Backup{
			ID:          "backup-1",
			Filename:    "backup.tar.gz.enc",
			Size:        1024,
			Checksum:    "abc",
			DBDriver:    "mysql",
			Status:      "completed",
			DownloadURL: "https://s3.example.com/download",
		})
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	backup, err := client.GetBackup("backup-1")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if backup.ID != "backup-1" {
		t.Errorf("unexpected id: %s", backup.ID)
	}

	if backup.DownloadURL != "https://s3.example.com/download" {
		t.Errorf("unexpected download url: %s", backup.DownloadURL)
	}
}

func TestDeleteBackup(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodDelete {
			t.Errorf("expected DELETE, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/backups/backup-1" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		w.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	err := client.DeleteBackup("backup-1")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestGetAccount(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			t.Errorf("expected GET, got %s", r.Method)
		}

		if r.URL.Path != "/api/v1/account" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}

		json.NewEncoder(w).Encode(AccountInfo{
			BackupCount:      42,
			StorageUsedBytes: 1048576,
			LastBackupAt:     "2026-03-22T10:00:00Z",
		})
	}))
	defer server.Close()

	client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
	info, err := client.GetAccount()

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if info.BackupCount != 42 {
		t.Errorf("unexpected backup count: %d", info.BackupCount)
	}

	if info.StorageUsedBytes != 1048576 {
		t.Errorf("unexpected storage: %d", info.StorageUsedBytes)
	}
}

func TestErrorMapping(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		body       string
		wantErr    error
	}{
		{"401 auth error", 401, `{"message":"Unauthorized"}`, ErrAuthentication},
		{"402 billing error", 402, `{"message":"Payment required"}`, ErrBilling},
		{"413 size limit", 413, `{"message":"Too large"}`, ErrSizeLimit},
		{"422 validation", 422, `{"message":"Invalid","details":{"field":"required"}}`, ErrValidation},
		{"429 rate limit", 429, `{"message":"Too many requests"}`, ErrRateLimit},
		{"500 server error", 500, `{"message":"Internal server error"}`, ErrServer},
		{"502 server error", 502, `{"message":"Bad gateway"}`, ErrServer},
		{"503 server error", 503, `{"message":"Service unavailable"}`, ErrServer},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.statusCode)
				io.WriteString(w, tt.body)
			}))
			defer server.Close()

			client := NewClient("test-key", server.URL+"/api/v1", "1.0.0")
			_, err := client.GetAccount()

			if err == nil {
				t.Fatal("expected error, got nil")
			}

			apiErr, ok := err.(*APIError)
			if !ok {
				t.Fatalf("expected *APIError, got %T", err)
			}

			if apiErr.StatusCode != tt.statusCode {
				t.Errorf("expected status %d, got %d", tt.statusCode, apiErr.StatusCode)
			}

			if !apiErr.Is(tt.wantErr) {
				t.Errorf("expected error to match %v", tt.wantErr)
			}
		})
	}
}

func TestConnectionError(t *testing.T) {
	client := NewClient("test-key", "http://localhost:1/api/v1", "1.0.0")
	_, err := client.GetAccount()

	if err == nil {
		t.Fatal("expected error for connection failure")
	}
}
