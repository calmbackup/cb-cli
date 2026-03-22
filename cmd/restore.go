package cmd

import (
	"fmt"
	"os"

	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/spf13/cobra"
)

func newRestoreCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "restore [backup-id]",
		Short: "Restore a backup",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			svc, err := buildService()
			if err != nil {
				return err
			}

			backupID := args[0]

			var progress backup.ProgressFunc
			if !quiet {
				progress = func(msg string) {
					fmt.Println(msg)
				}
			}

			if err := svc.Restore(backupID, progress); err != nil {
				fmt.Fprintf(os.Stderr, "Restore failed: %v\n", err)
				os.Exit(1)
			}

			if !quiet {
				fmt.Println("Restore completed successfully.")
			}

			return nil
		},
	}
}
