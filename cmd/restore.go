package cmd

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/spf13/cobra"
	"golang.org/x/term"
)

func newRestoreCmd() *cobra.Command {
	var latest, yes bool

	cmd := &cobra.Command{
		Use:   "restore [backup-id]",
		Short: "Restore a backup",
		Args:  cobra.MaximumNArgs(1),
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

			var backupID string

			switch {
			// Mode A: --latest flag
			case latest:
				if len(args) > 0 {
					return fmt.Errorf("cannot use --latest with a backup ID")
				}

				resp, err := svc.API.ListBackups(1, 1)
				if err != nil {
					return fmt.Errorf("failed to list backups: %w", err)
				}
				if len(resp.Data) == 0 {
					fmt.Println("No backups found. Run 'calmbackup run' to create one.")
					return nil
				}

				entry := resp.Data[0]
				fmt.Printf("Restoring latest backup: %s (%s, %s)\n", entry.Filename, formatSize(entry.Size), entry.CreatedAt)

				if !yes && !confirmRestore(entry.Filename) {
					fmt.Println("Restore cancelled.")
					return nil
				}

				backupID = entry.ID

			// Mode C: positional arg provided
			case len(args) == 1:
				backupID = args[0]

			// Mode B: interactive picker
			default:
				if !term.IsTerminal(int(os.Stdin.Fd())) || quiet {
					return fmt.Errorf("no backup ID specified; use --latest or provide a backup ID")
				}

				resp, err := svc.API.ListBackups(1, 50)
				if err != nil {
					return fmt.Errorf("failed to list backups: %w", err)
				}
				if len(resp.Data) == 0 {
					fmt.Println("No backups found. Run 'calmbackup run' to create one.")
					return nil
				}

				selected, err := runPicker(resp.Data)
				if err != nil {
					return err
				}
				if selected == nil {
					return nil
				}

				fmt.Printf("Selected backup: %s (%s, %s)\n", selected.Filename, formatSize(selected.Size), selected.CreatedAt)

				if !yes && !confirmRestore(selected.Filename) {
					fmt.Println("Restore cancelled.")
					return nil
				}

				backupID = selected.ID
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

	cmd.Flags().BoolVar(&latest, "latest", false, "restore the most recent backup")
	cmd.Flags().BoolVarP(&yes, "yes", "y", false, "skip confirmation prompt")

	return cmd
}

func confirmRestore(filename string) bool {
	fmt.Printf("Restore %s? This will overwrite your current database. [y/N] ", filename)
	scanner := bufio.NewScanner(os.Stdin)
	if !scanner.Scan() {
		return false
	}
	answer := strings.TrimSpace(strings.ToLower(scanner.Text()))
	return answer == "y" || answer == "yes"
}
