package cmd

import (
	"fmt"
	"os"

	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/calmbackup/cb-cli/internal/updater"
	"github.com/spf13/cobra"
)

func newRunCmd(version string) *cobra.Command {
	return &cobra.Command{
		Use:   "run",
		Short: "Run a backup",
		RunE: func(cmd *cobra.Command, args []string) error {
			svc, err := buildService()
			if err != nil {
				return err
			}

			var progress backup.ProgressFunc
			if !quiet {
				progress = func(msg string) {
					fmt.Println(msg)
				}
			}

			result := svc.Backup(progress)
			if !result.Success {
				fmt.Fprintf(os.Stderr, "Backup failed: %s\n", result.Error)
				os.Exit(1)
			}

			if !quiet {
				fmt.Printf("Backup successful: %s (%d bytes, %s)\n", result.Filename, result.Size, result.Duration)
			}

			// Auto-update after successful backup
			autoUpdate(version, progress)

			return nil
		},
	}
}

func autoUpdate(version string, progress backup.ProgressFunc) {
	if version == "dev" {
		return
	}

	latestTag, needsUpdate, err := updater.Check(version)
	if err != nil {
		if progress != nil {
			progress(fmt.Sprintf("Update check failed: %v", err))
		}

		return
	}

	if !needsUpdate {
		return
	}

	if progress != nil {
		progress(fmt.Sprintf("Updating %s → %s...", version, latestTag))
	}

	newTag, err := updater.Update(version)
	if err != nil {
		if progress != nil {
			progress(fmt.Sprintf("Auto-update failed: %v", err))
		}

		return
	}

	if progress != nil {
		progress(fmt.Sprintf("Updated to %s", newTag))
	}
}
