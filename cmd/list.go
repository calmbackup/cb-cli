package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"text/tabwriter"

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

			w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)

			// Local backups
			fmt.Fprintln(w, "LOCAL BACKUPS")
			fmt.Fprintln(w, "FILENAME\tSIZE")
			entries, err := os.ReadDir(svc.Config.LocalPath)
			if err == nil {
				for _, entry := range entries {
					if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".tar.gz.enc") {
						continue
					}

					info, err := entry.Info()
					if err != nil {
						continue
					}

					fmt.Fprintf(w, "%s\t%s\n", entry.Name(), formatSize(info.Size()))
				}
			}

			fmt.Fprintln(w)

			// Cloud backups
			fmt.Fprintln(w, "CLOUD BACKUPS")
			fmt.Fprintln(w, "FILENAME")
			resp, err := svc.API.ListBackups(1, 50)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Warning: could not list cloud backups: %v\n", err)
			} else {
				for _, entry := range resp.Data {
					fmt.Fprintln(w, entry.Filename)
				}
			}

			w.Flush()
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
