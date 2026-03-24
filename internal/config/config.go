package config

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

const (
	SystemConfigDir  = "/etc/calmbackup"
	SystemConfigFile = "/etc/calmbackup/calmbackup.yaml"
	DefaultLocalPath = "/var/backups/calmbackup"
)

// LocalPath returns the appropriate backup directory based on effective user.
// Root gets /var/backups/calmbackup, regular users get ~/.local/share/calmbackup.
func LocalPath() string {
	if os.Geteuid() == 0 {
		return DefaultLocalPath
	}

	home, err := os.UserHomeDir()
	if err != nil {
		return DefaultLocalPath
	}

	return filepath.Join(home, ".local", "share", "calmbackup")
}

type Config struct {
	APIKey             string   `yaml:"api_key"`
	EncryptionKey      string   `yaml:"encryption_key"`
	APIURL             string   `yaml:"api_url"`
	Database           DBConfig `yaml:"database"`
	Directories        []string `yaml:"directories"`
	LocalPath          string   `yaml:"local_path"`
	LocalRetentionDays int      `yaml:"local_retention_days"`
}

type DBConfig struct {
	Driver   string `yaml:"driver"`
	Host     string `yaml:"host"`
	Port     int    `yaml:"port"`
	Username string `yaml:"username"`
	Password string `yaml:"password"`
	Database string `yaml:"database"`
	Path     string `yaml:"path"`
}

var validDrivers = map[string]bool{
	"sqlite": true,
	"mysql":  true,
	"pgsql":  true,
}

func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading config file: %w", err)
	}

	var cfg Config
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parsing config file: %w", err)
	}

	cfg.applyDefaults()

	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	return &cfg, nil
}

func (c *Config) applyDefaults() {
	if c.APIURL == "" {
		c.APIURL = "https://app.calmbackup.com/api/v1"
	}
	if c.LocalRetentionDays == 0 {
		c.LocalRetentionDays = 7
	}
	if c.LocalPath == "" {
		c.LocalPath = LocalPath()
	}
}

func (c *Config) Validate() error {
	if c.APIKey == "" {
		return fmt.Errorf("api_key is required")
	}
	if c.EncryptionKey == "" {
		return fmt.Errorf("encryption_key is required")
	}
	if c.Database.Driver == "" {
		return fmt.Errorf("database.driver is required")
	}
	if !validDrivers[c.Database.Driver] {
		return fmt.Errorf("database.driver must be one of: sqlite, mysql, pgsql (got %q)", c.Database.Driver)
	}
	return nil
}

// LoadPartial loads config without validation (for notification when config is incomplete).
func LoadPartial(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var cfg Config
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}

	cfg.applyDefaults()

	return &cfg, nil
}

func FindConfigFile() (string, error) {
	// 1. Check /etc/calmbackup/calmbackup.yaml (system-wide, most common for servers)
	if _, err := os.Stat(SystemConfigFile); err == nil {
		return SystemConfigFile, nil
	}

	// 2. Check $HOME/.config/calmbackup/calmbackup.yaml
	home, err := os.UserHomeDir()
	if err == nil {
		homePath := filepath.Join(home, ".config", "calmbackup", "calmbackup.yaml")
		if _, err := os.Stat(homePath); err == nil {
			return homePath, nil
		}
	}

	// 3. Check current directory
	localPath := "calmbackup.yaml"
	if _, err := os.Stat(localPath); err == nil {
		return filepath.Abs(localPath)
	}

	return "", fmt.Errorf("config file not found: searched /etc/calmbackup/, ~/.config/calmbackup/, and ./calmbackup.yaml")
}

// ConfigDir returns the appropriate config directory based on effective user.
// Root gets /etc/calmbackup, regular users get ~/.config/calmbackup.
func ConfigDir() string {
	if os.Geteuid() == 0 {
		return SystemConfigDir
	}

	home, err := os.UserHomeDir()
	if err != nil {
		return "."
	}

	return filepath.Join(home, ".config", "calmbackup")
}
