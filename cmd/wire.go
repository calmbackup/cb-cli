package cmd

import (
	"fmt"

	"github.com/calmbackup/cb-cli/internal/api"
	"github.com/calmbackup/cb-cli/internal/archive"
	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/calmbackup/cb-cli/internal/config"
	"github.com/calmbackup/cb-cli/internal/crypto"
	"github.com/calmbackup/cb-cli/internal/dumper"
	"github.com/calmbackup/cb-cli/internal/prune"
	"github.com/calmbackup/cb-cli/internal/upload"
)

// wiredService wraps backup.Service with the loaded config for status display.
type wiredService struct {
	*backup.Service
	Config *config.Config
}

func buildService() (*wiredService, error) {
	var configPath string
	if cfgFile != "" {
		configPath = cfgFile
	} else {
		found, err := config.FindConfigFile()
		if err != nil {
			return nil, fmt.Errorf("config file not found: %w (use --config or run 'calmbackup init')", err)
		}
		configPath = found
	}

	cfg, err := config.Load(configPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}

	dbDumper, err := dumper.NewDumper(dumper.DBConfig{
		Driver:   cfg.Database.Driver,
		Host:     cfg.Database.Host,
		Port:     cfg.Database.Port,
		Username: cfg.Database.Username,
		Password: cfg.Database.Password,
		Database: cfg.Database.Database,
		Path:     cfg.Database.Path,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create database dumper: %w", err)
	}

	encKey := crypto.DeriveKey(cfg.EncryptionKey)

	svc := &backup.Service{
		Dumper:    dbDumper,
		Archiver:  &archiverAdapter{},
		Encryptor: &encryptorAdapter{key: encKey},
		API:       &apiAdapter{client: api.NewClient(cfg.APIKey, cfg.APIURL, "dev")},
		Uploader:  &uploaderAdapter{},
		Pruner:    &prunerAdapter{},
		Config: backup.ServiceConfig{
			DBDriver:      cfg.Database.Driver,
			Directories:   cfg.Directories,
			LocalPath:     cfg.LocalPath,
			RetentionDays: cfg.LocalRetentionDays,
		},
	}

	return &wiredService{Service: svc, Config: cfg}, nil
}

// Adapters to bridge package-level functions to the backup interfaces.

type archiverAdapter struct{}

func (a *archiverAdapter) Create(dumpPath string, directories []string, outputPath string) error {
	return archive.Create(dumpPath, directories, outputPath)
}

func (a *archiverAdapter) Extract(archivePath string, outputDir string) ([]string, error) {
	return archive.Extract(archivePath, outputDir)
}

type encryptorAdapter struct {
	key []byte
}

func (e *encryptorAdapter) Encrypt(inputPath, outputPath string) error {
	return crypto.Encrypt(inputPath, outputPath, e.key)
}

func (e *encryptorAdapter) Decrypt(inputPath, outputPath string) error {
	return crypto.Decrypt(inputPath, outputPath, e.key)
}

type apiAdapter struct {
	client *api.Client
}

func (a *apiAdapter) RequestUploadURL(filename string, size int64, checksum, dbDriver string) (*backup.UploadURLResponse, error) {
	resp, err := a.client.RequestUploadURL(filename, size, checksum, dbDriver)
	if err != nil {
		return nil, err
	}
	return &backup.UploadURLResponse{
		BackupID:  resp.BackupID,
		UploadURL: resp.UploadURL,
	}, nil
}

func (a *apiAdapter) ConfirmBackup(backupID string, size int64, checksum string) error {
	return a.client.ConfirmBackup(backupID, size, checksum)
}

func (a *apiAdapter) ListBackups(page, perPage int) (*backup.ListBackupsResponse, error) {
	resp, err := a.client.ListBackups(page, perPage)
	if err != nil {
		return nil, err
	}
	entries := make([]backup.BackupEntry, len(resp.Data))
	for i, b := range resp.Data {
		entries[i] = backup.BackupEntry{
			Filename:    b.Filename,
			DownloadURL: b.DownloadURL,
		}
	}
	return &backup.ListBackupsResponse{Data: entries}, nil
}

func (a *apiAdapter) GetBackup(backupID string) (*backup.BackupDetail, error) {
	b, err := a.client.GetBackup(backupID)
	if err != nil {
		return nil, err
	}
	return &backup.BackupDetail{
		Filename:    b.Filename,
		DownloadURL: b.DownloadURL,
	}, nil
}

type uploaderAdapter struct{}

func (u *uploaderAdapter) Upload(filePath, presignedURL string) error {
	return upload.Upload(filePath, presignedURL)
}

func (u *uploaderAdapter) Download(presignedURL, outputPath string) error {
	return upload.Download(presignedURL, outputPath)
}

type prunerAdapter struct{}

func (p *prunerAdapter) Prune(backupDir string, retentionDays int, confirmedFilenames []string) (int, error) {
	return prune.Prune(backupDir, retentionDays, confirmedFilenames)
}
