package cmd

import (
	"fmt"
	"os"

	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/spf13/cobra"
)

func newRunCmd() *cobra.Command {
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

			return nil
		},
	}
}
