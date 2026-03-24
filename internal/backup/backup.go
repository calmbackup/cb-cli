package backup

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/calmbackup/cb-cli/internal/api"
)

// ProgressFunc is called with status messages during backup/restore.
type ProgressFunc func(message string)

// Result holds the outcome of a backup operation.
type Result struct {
	Success  bool
	Filename string
	Size     int64
	Duration time.Duration
	Error    string
}

// DatabaseDumper dumps, verifies, and restores database files.
type DatabaseDumper interface {
	Dump(outputPath string) error
	Verify(dumpPath string) (bool, error)
	Restore(dumpPath string) error
	Filename() string
}

// Archiver creates and extracts tar.gz archives.
type Archiver interface {
	Create(dumpPath string, directories []string, outputPath string) error
	Extract(archivePath string, outputDir string) ([]string, error)
}

// Encryptor encrypts and decrypts files.
type Encryptor interface {
	Encrypt(inputPath, outputPath string) error
	Decrypt(inputPath, outputPath string) error
}

// APIClient communicates with the CalmBackup API.
type APIClient interface {
	RequestUploadURL(filename string, size int64, checksum, dbDriver string) (*UploadURLResponse, error)
	ConfirmBackup(backupID string, size int64, checksum string) error
	ListBackups(page, perPage int) (*ListBackupsResponse, error)
	GetBackup(backupID string) (*BackupDetail, error)
}

// Uploader handles presigned URL uploads and downloads.
type Uploader interface {
	Upload(filePath, presignedURL string) error
	Download(presignedURL, outputPath string) error
}

// Pruner removes old local backups.
type Pruner interface {
	Prune(backupDir string, retentionDays int, confirmedFilenames []string) (int, error)
}

// UploadURLResponse from the API.
type UploadURLResponse struct {
	BackupID  string
	UploadURL string
}

// ListBackupsResponse from the API.
type ListBackupsResponse struct {
	Data []BackupEntry
}

// BackupEntry represents a single backup in a list.
type BackupEntry struct {
	ID          string
	Filename    string
	Size        int64
	CreatedAt   string
	DownloadURL string
}

// BackupDetail represents detailed backup info.
type BackupDetail struct {
	Filename    string
	Checksum    string
	DownloadURL string
}

// ServiceConfig holds configuration for the backup service.
type ServiceConfig struct {
	DBDriver      string
	Directories   []string
	LocalPath     string
	RetentionDays int
}

// Service orchestrates backup and restore operations.
type Service struct {
	Dumper    DatabaseDumper
	Archiver  Archiver
	Encryptor Encryptor
	API       APIClient
	Uploader  Uploader
	Pruner    Pruner
	Config    ServiceConfig
}

func (s *Service) progress(fn ProgressFunc, msg string) {
	if fn != nil {
		fn(msg)
	}
}

// Backup performs a full backup: dump, archive, encrypt, upload, prune.
func (s *Service) Backup(onProgress ProgressFunc) Result {
	start := time.Now()

	// 1. Create temp dir
	tmpDir, err := os.MkdirTemp("", "calmbackup-*")
	if err != nil {
		return Result{Error: fmt.Sprintf("failed to create temp dir: %v", err)}
	}
	defer os.RemoveAll(tmpDir)

	// 2. Dump database
	s.progress(onProgress, "Dumping database...")
	dumpPath := filepath.Join(tmpDir, s.Dumper.Filename())
	if err := s.Dumper.Dump(dumpPath); err != nil {
		return Result{Error: fmt.Sprintf("database dump failed: %v", err)}
	}

	// 3. Verify dump
	s.progress(onProgress, "Verifying dump...")
	ok, err := s.Dumper.Verify(dumpPath)
	if err != nil {
		return Result{Error: fmt.Sprintf("dump verification error: %v", err)}
	}
	if !ok {
		return Result{Error: "dump verification failed: dump appears invalid"}
	}

	// 4. Archive (dump + directories)
	s.progress(onProgress, "Creating archive...")
	archivePath := filepath.Join(tmpDir, "backup.tar.gz")
	if err := s.Archiver.Create(dumpPath, s.Config.Directories, archivePath); err != nil {
		return Result{Error: fmt.Sprintf("archive creation failed: %v", err)}
	}

	// 5. Encrypt
	s.progress(onProgress, "Encrypting backup...")
	timestamp := time.Now().Format("20060102-150405")
	encryptedFilename := fmt.Sprintf("backup-%s.tar.gz.enc", timestamp)
	encryptedPath := filepath.Join(tmpDir, encryptedFilename)
	if err := s.Encryptor.Encrypt(archivePath, encryptedPath); err != nil {
		return Result{Error: fmt.Sprintf("encryption failed: %v", err)}
	}

	// 6. Catch-up upload (local files not in cloud, before saving new file)
	s.progress(onProgress, "Checking for un-uploaded backups...")
	s.catchUpUpload(onProgress)

	// 7. Copy to local path
	s.progress(onProgress, "Saving local copy...")
	if err := os.MkdirAll(s.Config.LocalPath, 0755); err != nil {
		return Result{Error: fmt.Sprintf("failed to create local path: %v", err)}
	}
	localDest := filepath.Join(s.Config.LocalPath, encryptedFilename)
	if err := copyFile(encryptedPath, localDest); err != nil {
		return Result{Error: fmt.Sprintf("failed to copy to local path: %v", err)}
	}

	// 8. Compute SHA-256 checksum
	checksum, err := sha256sum(localDest)
	if err != nil {
		return Result{Error: fmt.Sprintf("failed to compute checksum: %v", err)}
	}

	fileInfo, err := os.Stat(localDest)
	if err != nil {
		return Result{Error: fmt.Sprintf("failed to stat local file: %v", err)}
	}

	// 9. Request upload URL, upload, confirm
	s.progress(onProgress, "Uploading backup...")
	resp, err := s.API.RequestUploadURL(encryptedFilename, fileInfo.Size(), checksum, s.Config.DBDriver)
	if errors.Is(err, api.ErrBackupDeleted) {
		s.progress(onProgress, "Skipped: backup was previously deleted from cloud")
	} else if err != nil {
		// Upload failure is non-fatal; local backup still exists
		s.progress(onProgress, fmt.Sprintf("Warning: upload request failed: %v", err))
	} else {
		if err := s.Uploader.Upload(localDest, resp.UploadURL); err != nil {
			s.progress(onProgress, fmt.Sprintf("Warning: upload failed: %v", err))
		} else {
			if err := s.API.ConfirmBackup(resp.BackupID, fileInfo.Size(), checksum); err != nil {
				s.progress(onProgress, fmt.Sprintf("Warning: backup confirmation failed: %v", err))
			}
		}
	}

	// 10. Prune old local backups
	s.progress(onProgress, "Pruning old backups...")
	cloudFilenames := s.getCloudFilenames()
	pruned, err := s.Pruner.Prune(s.Config.LocalPath, s.Config.RetentionDays, cloudFilenames)
	if err != nil {
		s.progress(onProgress, fmt.Sprintf("Warning: pruning failed: %v", err))
	} else if pruned > 0 {
		s.progress(onProgress, fmt.Sprintf("Pruned %d old backup(s)", pruned))
	}

	s.progress(onProgress, "Backup complete!")

	return Result{
		Success:  true,
		Filename: encryptedFilename,
		Size:     fileInfo.Size(),
		Duration: time.Since(start),
	}
}

func (s *Service) catchUpUpload(onProgress ProgressFunc) {
	// List cloud backups
	cloudResp, err := s.API.ListBackups(1, 100)
	if err != nil {
		s.progress(onProgress, fmt.Sprintf("Warning: could not list cloud backups: %v", err))
		return
	}

	cloudFiles := make(map[string]bool)
	for _, entry := range cloudResp.Data {
		cloudFiles[entry.Filename] = true
	}

	// Find local .enc files not in cloud
	entries, err := os.ReadDir(s.Config.LocalPath)
	if err != nil {
		return
	}

	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".tar.gz.enc") {
			continue
		}
		if cloudFiles[entry.Name()] {
			continue
		}

		localPath := filepath.Join(s.Config.LocalPath, entry.Name())
		info, err := entry.Info()
		if err != nil {
			continue
		}

		checksum, err := sha256sum(localPath)
		if err != nil {
			continue
		}

		s.progress(onProgress, fmt.Sprintf("Catch-up uploading %s...", entry.Name()))
		resp, err := s.API.RequestUploadURL(entry.Name(), info.Size(), checksum, s.Config.DBDriver)
		if errors.Is(err, api.ErrBackupDeleted) {
			s.progress(onProgress, fmt.Sprintf("Skipped %s: previously deleted from cloud", entry.Name()))
			continue
		}
		if err != nil {
			s.progress(onProgress, fmt.Sprintf("Warning: catch-up upload request failed for %s: %v", entry.Name(), err))
			continue
		}

		if err := s.Uploader.Upload(localPath, resp.UploadURL); err != nil {
			s.progress(onProgress, fmt.Sprintf("Warning: catch-up upload failed for %s: %v", entry.Name(), err))
			continue
		}

		if err := s.API.ConfirmBackup(resp.BackupID, info.Size(), checksum); err != nil {
			s.progress(onProgress, fmt.Sprintf("Warning: catch-up confirmation failed for %s: %v", entry.Name(), err))
		}
	}
}

func (s *Service) getCloudFilenames() []string {
	resp, err := s.API.ListBackups(1, 100)
	if err != nil {
		return nil
	}
	names := make([]string, len(resp.Data))
	for i, entry := range resp.Data {
		names[i] = entry.Filename
	}
	return names
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()

	if _, err := io.Copy(out, in); err != nil {
		return err
	}

	return out.Close()
}

func sha256sum(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()

	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}

	return fmt.Sprintf("%x", h.Sum(nil)), nil
}
