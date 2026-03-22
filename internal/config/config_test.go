package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoad_ValidConfig(t *testing.T) {
	yaml := `
api_key: "test-api-key"
encryption_key: "test-encryption-key"
api_url: "https://custom.api.com/v1"
database:
  driver: sqlite
  path: "/tmp/test.db"
directories:
  - /home/user/docs
  - /var/data
local_path: "/backups/custom"
local_retention_days: 14
`
	path := writeTempYAML(t, yaml)
	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if cfg.APIKey != "test-api-key" {
		t.Errorf("expected api_key 'test-api-key', got %q", cfg.APIKey)
	}
	if cfg.EncryptionKey != "test-encryption-key" {
		t.Errorf("expected encryption_key 'test-encryption-key', got %q", cfg.EncryptionKey)
	}
	if cfg.APIURL != "https://custom.api.com/v1" {
		t.Errorf("expected custom api_url, got %q", cfg.APIURL)
	}
	if cfg.Database.Driver != "sqlite" {
		t.Errorf("expected driver sqlite, got %q", cfg.Database.Driver)
	}
	if cfg.Database.Path != "/tmp/test.db" {
		t.Errorf("expected database path /tmp/test.db, got %q", cfg.Database.Path)
	}
	if len(cfg.Directories) != 2 {
		t.Errorf("expected 2 directories, got %d", len(cfg.Directories))
	}
	if cfg.LocalPath != "/backups/custom" {
		t.Errorf("expected local_path '/backups/custom', got %q", cfg.LocalPath)
	}
	if cfg.LocalRetentionDays != 14 {
		t.Errorf("expected local_retention_days 14, got %d", cfg.LocalRetentionDays)
	}
}

func TestLoad_MissingAPIKey(t *testing.T) {
	yaml := `
encryption_key: "test-key"
database:
  driver: sqlite
  path: "/tmp/test.db"
`
	path := writeTempYAML(t, yaml)
	_, err := Load(path)
	if err == nil {
		t.Fatal("expected error for missing api_key")
	}
}

func TestLoad_MissingEncryptionKey(t *testing.T) {
	yaml := `
api_key: "test-key"
database:
  driver: sqlite
  path: "/tmp/test.db"
`
	path := writeTempYAML(t, yaml)
	_, err := Load(path)
	if err == nil {
		t.Fatal("expected error for missing encryption_key")
	}
}

func TestLoad_MissingDatabase(t *testing.T) {
	yaml := `
api_key: "test-key"
encryption_key: "test-enc-key"
`
	path := writeTempYAML(t, yaml)
	_, err := Load(path)
	if err == nil {
		t.Fatal("expected error for missing database driver")
	}
}

func TestLoad_InvalidDriver(t *testing.T) {
	yaml := `
api_key: "test-key"
encryption_key: "test-enc-key"
database:
  driver: "oracle"
  host: "localhost"
`
	path := writeTempYAML(t, yaml)
	_, err := Load(path)
	if err == nil {
		t.Fatal("expected error for invalid driver")
	}
}

func TestLoad_ValidDrivers(t *testing.T) {
	drivers := []string{"sqlite", "mysql", "pgsql"}
	for _, driver := range drivers {
		yaml := `
api_key: "test-key"
encryption_key: "test-enc-key"
database:
  driver: "` + driver + `"
  path: "/tmp/test.db"
`
		path := writeTempYAML(t, yaml)
		_, err := Load(path)
		if err != nil {
			t.Errorf("expected driver %q to be valid, got error: %v", driver, err)
		}
	}
}

func TestLoad_DefaultsApplied(t *testing.T) {
	yaml := `
api_key: "test-key"
encryption_key: "test-enc-key"
database:
  driver: sqlite
  path: "/tmp/test.db"
`
	path := writeTempYAML(t, yaml)
	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if cfg.APIURL != "https://app.calmbackup.com/api/v1" {
		t.Errorf("expected default api_url, got %q", cfg.APIURL)
	}
	if cfg.LocalRetentionDays != 7 {
		t.Errorf("expected default local_retention_days 7, got %d", cfg.LocalRetentionDays)
	}
	if cfg.LocalPath != "/var/backups/calmbackup" {
		t.Errorf("expected default local_path '/var/backups/calmbackup', got %q", cfg.LocalPath)
	}
}

func TestLoad_FileNotFound(t *testing.T) {
	_, err := Load("/nonexistent/path/config.yaml")
	if err == nil {
		t.Fatal("expected error for nonexistent file")
	}
}

func TestLoad_InvalidYAML(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.yaml")
	if err := os.WriteFile(path, []byte(":::invalid yaml:::"), 0644); err != nil {
		t.Fatal(err)
	}
	_, err := Load(path)
	if err == nil {
		t.Fatal("expected error for invalid YAML")
	}
}

func TestFindConfigFile_CurrentDir(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "calmbackup.yaml")
	if err := os.WriteFile(cfgPath, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	origDir, _ := os.Getwd()
	defer os.Chdir(origDir)
	os.Chdir(dir)

	found, err := FindConfigFile()
	if err != nil {
		t.Fatalf("expected to find config, got error: %v", err)
	}
	if filepath.Base(found) != "calmbackup.yaml" {
		t.Errorf("expected calmbackup.yaml, got %q", found)
	}
}

func TestFindConfigFile_HomeConfig(t *testing.T) {
	dir := t.TempDir()

	// Create config in fake home dir
	configDir := filepath.Join(dir, ".config", "calmbackup")
	if err := os.MkdirAll(configDir, 0755); err != nil {
		t.Fatal(err)
	}
	cfgPath := filepath.Join(configDir, "calmbackup.yaml")
	if err := os.WriteFile(cfgPath, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	// Set HOME and change to a dir without config
	emptyDir := t.TempDir()
	origDir, _ := os.Getwd()
	origHome := os.Getenv("HOME")
	defer func() {
		os.Chdir(origDir)
		os.Setenv("HOME", origHome)
	}()
	os.Chdir(emptyDir)
	os.Setenv("HOME", dir)

	found, err := FindConfigFile()
	if err != nil {
		t.Fatalf("expected to find config, got error: %v", err)
	}
	if found != cfgPath {
		t.Errorf("expected %q, got %q", cfgPath, found)
	}
}

func TestFindConfigFile_NotFound(t *testing.T) {
	dir := t.TempDir()
	origDir, _ := os.Getwd()
	origHome := os.Getenv("HOME")
	defer func() {
		os.Chdir(origDir)
		os.Setenv("HOME", origHome)
	}()
	os.Chdir(dir)
	os.Setenv("HOME", dir)

	_, err := FindConfigFile()
	if err == nil {
		t.Fatal("expected error when config file not found")
	}
}

func writeTempYAML(t *testing.T, content string) string {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "calmbackup.yaml")
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
	return path
}
