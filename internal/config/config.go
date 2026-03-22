package config

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

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
		c.LocalPath = "backups"
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

func FindConfigFile() (string, error) {
	// Check current directory first
	localPath := "calmbackup.yaml"
	if _, err := os.Stat(localPath); err == nil {
		return filepath.Abs(localPath)
	}

	// Check $HOME/.config/calmbackup/calmbackup.yaml
	home, err := os.UserHomeDir()
	if err == nil {
		homePath := filepath.Join(home, ".config", "calmbackup", "calmbackup.yaml")
		if _, err := os.Stat(homePath); err == nil {
			return homePath, nil
		}
	}

	return "", fmt.Errorf("config file not found: searched ./calmbackup.yaml and ~/.config/calmbackup/calmbackup.yaml")
}
