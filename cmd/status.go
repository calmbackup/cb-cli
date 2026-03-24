package cmd

import (
	"fmt"

	"github.com/spf13/cobra"
)

func newStatusCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "status",
		Short: "Show backup status and connectivity",
		RunE: func(cmd *cobra.Command, args []string) error {
			svc, err := buildService()
			if err != nil {
				return err
			}

			// Local backup info
			count := localBackupCount(svc.Config.LocalPath)
			size := localBackupSize(svc.Config.LocalPath)
			latest := latestBackupFile(svc.Config.LocalPath)

			fmt.Println()
			printLabel("Local Backups", fmt.Sprintf("%d (%s)", count, formatSize(size)))
			printLabel("Latest", latest)
			printLabel("Local Path", svc.Config.LocalPath)
			printLabel("Retention", fmt.Sprintf("%d days", svc.Config.RetentionDays))

			// API connectivity
			_, err = svc.API.ListBackups(1, 1)
			if err != nil {
				printLabel("API Connection", "✗ Failed — "+err.Error())
			} else {
				printLabel("API Connection", successStyle.Render("✓ Connected"))
			}

			fmt.Println()
			return nil
		},
	}
}
