package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/spf13/cobra"
)

func newListCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "list",
		Short: "List local and cloud backups",
		RunE: func(cmd *cobra.Command, args []string) error {
			svc, err := buildService()
			if err != nil {
				return err
			}

			// Local backups
			printSection("Local Backups")
			entries, err := os.ReadDir(svc.Config.LocalPath)
			localFound := false
			if err == nil {
				for _, entry := range entries {
					if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".tar.gz.enc") {
						continue
					}

					info, err := entry.Info()
					if err != nil {
						continue
					}

					localFound = true
					printInfo(fmt.Sprintf("🔒 %-45s  %s", entry.Name(), formatSize(info.Size())))
				}
			}
			if !localFound {
				printInfo(dimStyle.Render("No local backups"))
			}

			// Cloud backups
			printSection("Cloud Backups")
			resp, err := svc.API.ListBackups(1, 50)
			if err != nil {
				printInfo(dimStyle.Render("Could not list cloud backups"))
			} else if len(resp.Data) == 0 {
				printInfo(dimStyle.Render("No cloud backups"))
			} else {
				for _, entry := range resp.Data {
					ts := formatCloudTime(entry.CreatedAt)
					printInfo(fmt.Sprintf("🔒 %-45s  %s  %s", entry.Filename, formatSize(entry.Size), ts))
				}
			}

			fmt.Println()
			return nil
		},
	}
}

func formatSize(bytes int64) string {
	const (
		kb = 1024
		mb = kb * 1024
		gb = mb * 1024
	)

	switch {
	case bytes >= gb:
		return fmt.Sprintf("%.1f GB", float64(bytes)/float64(gb))
	case bytes >= mb:
		return fmt.Sprintf("%.1f MB", float64(bytes)/float64(mb))
	case bytes >= kb:
		return fmt.Sprintf("%.1f KB", float64(bytes)/float64(kb))
	default:
		return fmt.Sprintf("%d B", bytes)
	}
}

// formatCloudTime parses a timestamp string and returns a short format like "Mar 24, 18:44".
func formatCloudTime(ts string) string {
	for _, layout := range []string{time.RFC3339, "2006-01-02T15:04:05.000000Z", "2006-01-02 15:04:05"} {
		if t, err := time.Parse(layout, ts); err == nil {
			return t.Format("Jan 02, 15:04")
		}
	}
	return ts
}

// localBackupCount returns the number of .tar.gz.enc files in a directory.
func localBackupCount(dir string) int {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return 0
	}

	count := 0
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".tar.gz.enc") {
			count++
		}
	}

	return count
}

// localBackupSize returns total size of .tar.gz.enc files in a directory.
func localBackupSize(dir string) int64 {
	var total int64

	entries, err := os.ReadDir(dir)
	if err != nil {
		return 0
	}

	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".tar.gz.enc") {
			continue
		}

		info, err := entry.Info()
		if err != nil {
			continue
		}

		total += info.Size()
	}

	return total
}

// latestBackupFile returns the name of the most recent .tar.gz.enc file.
func latestBackupFile(dir string) string {
	pattern := filepath.Join(dir, "*.tar.gz.enc")
	matches, err := filepath.Glob(pattern)
	if err != nil || len(matches) == 0 {
		return "none"
	}

	// Filenames contain timestamps, so lexicographic sort works
	latest := matches[0]
	for _, m := range matches[1:] {
		if filepath.Base(m) > filepath.Base(latest) {
			latest = m
		}
	}

	return filepath.Base(latest)
}
