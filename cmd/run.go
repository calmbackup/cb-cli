package cmd

import (
	"fmt"
	"os"
	"strings"

	"github.com/calmbackup/cb-cli/internal/api"
	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/calmbackup/cb-cli/internal/config"
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
				notifyBuildError(err)

				return err
			}

			var progress backup.ProgressFunc
			if !quiet {
				progress = func(msg string) {
					printStep(msg)
				}
			}

			result := svc.Backup(progress)
			if !result.Success {
				notifyBackupFailure(svc.FullConfig(), result.Error)
				fmt.Fprintf(os.Stderr, "Backup failed: %s\n", result.Error)
				os.Exit(1)
			}

			if !quiet {
				printDone(fmt.Sprintf("Backup successful: %s (%s, %s)", result.Filename, formatSize(result.Size), result.Duration))
			}

			return nil
		},
	}
}

// notifyBuildError sends a notification if the error is a missing encryption key.
func notifyBuildError(err error) {
	if !strings.Contains(err.Error(), "encryption_key") {
		return
	}

	// Try to load config partially just to get the API key for notification
	cfg := loadPartialConfig()
	if cfg == nil || cfg.APIKey == "" {
		return
	}

	client := api.NewClient(cfg.APIKey, cfg.APIURL, "")
	_ = client.Notify("key-missing", "")
}

// notifyBackupFailure sends a backup-failed notification via the API.
func notifyBackupFailure(cfg *config.Config, reason string) {
	if cfg == nil || cfg.APIKey == "" {
		return
	}

	client := api.NewClient(cfg.APIKey, cfg.APIURL, "")
	_ = client.Notify("backup-failed", reason)
}

// loadPartialConfig loads the config without validation for notification purposes.
func loadPartialConfig() *config.Config {
	var configPath string
	if cfgFile != "" {
		configPath = cfgFile
	} else {
		found, err := config.FindConfigFile()
		if err != nil {
			return nil
		}

		configPath = found
	}

	cfg, err := config.LoadPartial(configPath)
	if err != nil {
		return nil
	}

	return cfg
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
