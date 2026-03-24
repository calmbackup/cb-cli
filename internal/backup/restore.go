package backup

import (
	"fmt"
	"os"
	"path/filepath"
)

// Restore downloads (if needed), decrypts, extracts, and restores a backup.
func (s *Service) Restore(backupID string, onProgress ProgressFunc) error {
	// 1. Get backup details from API
	s.progress(onProgress, "Fetching backup details...")
	detail, err := s.API.GetBackup(backupID)
	if err != nil {
		return fmt.Errorf("failed to get backup details: %w", err)
	}

	// Create temp dir
	tmpDir, err := os.MkdirTemp("", "calmbackup-restore-*")
	if err != nil {
		return fmt.Errorf("failed to create temp dir: %w", err)
	}
	defer os.RemoveAll(tmpDir)

	encryptedPath := filepath.Join(tmpDir, detail.Filename)

	// 2. Check if file exists locally first
	localPath := filepath.Join(s.Config.LocalPath, detail.Filename)
	useLocal := false
	if _, err := os.Stat(localPath); err == nil {
		if detail.Checksum != "" {
			localChecksum, csErr := sha256sum(localPath)
			if csErr == nil && localChecksum == detail.Checksum {
				s.progress(onProgress, "Found locally — checksum verified, skipping download")
				useLocal = true
			} else {
				s.progress(onProgress, "Found locally — checksum mismatch, downloading from cloud")
			}
		} else {
			s.progress(onProgress, "Found locally — skipping download")
			useLocal = true
		}
	}

	if useLocal {
		if err := copyFile(localPath, encryptedPath); err != nil {
			return fmt.Errorf("failed to copy local file: %w", err)
		}
	} else {
		// 3. Download from cloud
		s.progress(onProgress, "Downloading from cloud...")
		if err := s.Uploader.Download(detail.DownloadURL, encryptedPath); err != nil {
			return fmt.Errorf("download failed: %w", err)
		}
	}

	// 4. Decrypt
	s.progress(onProgress, "Decrypting backup...")
	archivePath := filepath.Join(tmpDir, "backup.tar.gz")
	if err := s.Encryptor.Decrypt(encryptedPath, archivePath); err != nil {
		return fmt.Errorf("decryption failed: %w", err)
	}

	// 5. Extract archive
	s.progress(onProgress, "Extracting archive...")
	extractDir := filepath.Join(tmpDir, "extracted")
	if err := os.MkdirAll(extractDir, 0755); err != nil {
		return fmt.Errorf("failed to create extract dir: %w", err)
	}
	_, err = s.Archiver.Extract(archivePath, extractDir)
	if err != nil {
		return fmt.Errorf("extraction failed: %w", err)
	}

	// 6. Restore database
	s.progress(onProgress, "Restoring database...")
	dumpPath := filepath.Join(extractDir, s.Dumper.Filename())
	if err := s.Dumper.Restore(dumpPath); err != nil {
		return fmt.Errorf("database restore failed: %w", err)
	}

	// 7. Restore directories (only if configured)
	if len(s.Config.Directories) > 0 {
		s.progress(onProgress, "Restoring directories...")
		for _, dir := range s.Config.Directories {
			srcDir := filepath.Join(extractDir, dir)
			if _, err := os.Stat(srcDir); os.IsNotExist(err) {
				continue
			}
			if err := copyDir(srcDir, dir); err != nil {
				return fmt.Errorf("failed to restore directory %s: %w", dir, err)
			}
		}
	}

	s.progress(onProgress, "Restore complete!")
	return nil
}

func copyDir(src, dst string) error {
	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		relPath, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		dstPath := filepath.Join(dst, relPath)

		if info.IsDir() {
			return os.MkdirAll(dstPath, info.Mode())
		}

		return copyFile(path, dstPath)
	})
}
