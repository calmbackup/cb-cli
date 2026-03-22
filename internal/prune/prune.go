package prune

import (
	"fmt"
	"os"
	"path/filepath"
	"time"
)

func Prune(backupDir string, retentionDays int, confirmedFilenames []string) (int, error) {
	if _, err := os.Stat(backupDir); os.IsNotExist(err) {
		return 0, fmt.Errorf("directory does not exist: %s", backupDir)
	}

	matches, err := filepath.Glob(filepath.Join(backupDir, "*.tar.gz.enc"))
	if err != nil {
		return 0, fmt.Errorf("failed to glob files: %w", err)
	}

	confirmed := make(map[string]bool, len(confirmedFilenames))
	for _, name := range confirmedFilenames {
		confirmed[name] = true
	}

	cutoff := time.Now().Add(-time.Duration(retentionDays) * 24 * time.Hour)
	deleted := 0

	for _, path := range matches {
		info, err := os.Stat(path)
		if err != nil {
			continue
		}

		filename := filepath.Base(path)

		if info.ModTime().Before(cutoff) && confirmed[filename] {
			if err := os.Remove(path); err != nil {
				return deleted, fmt.Errorf("failed to delete %s: %w", filename, err)
			}

			deleted++
		}
	}

	return deleted, nil
}
