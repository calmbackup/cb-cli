package cmd

import (
	"fmt"
	"os"

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

			fmt.Printf("Local backups:  %d (%s)\n", count, formatSize(size))
			fmt.Printf("Latest backup:  %s\n", latest)
			fmt.Printf("Local path:     %s\n", svc.Config.LocalPath)
			fmt.Printf("Retention:      %d days\n", svc.Config.LocalRetentionDays)

			// API connectivity
			fmt.Print("API connection: ")
			_, err = svc.API.ListBackups(1, 1)
			if err != nil {
				fmt.Fprintf(os.Stderr, "FAILED (%v)\n", err)
			} else {
				fmt.Println("OK")
			}

			return nil
		},
	}
}
