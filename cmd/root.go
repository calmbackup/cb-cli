package cmd

import (
	"fmt"
	"os"

	"github.com/calmbackup/cb-cli/internal/backup"
	"github.com/spf13/cobra"
)

var (
	cfgFile string
	verbose bool
	quiet   bool
)

// NewRootCmd creates the root cobra command.
func NewRootCmd(version string) *cobra.Command {
	root := &cobra.Command{
		Use:   "calmbackup",
		Short: "Zero-knowledge encrypted backup CLI",
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			if !quiet {
				fmt.Fprintf(cmd.OutOrStdout(), "%s\n\n", brandHeader())
			}
		},
		PersistentPostRun: func(cmd *cobra.Command, args []string) {
			// Skip auto-update for commands that don't do real work
			name := cmd.Name()
			if name == "version" || name == "help" || name == "calmbackup" {
				return
			}

			var progress backup.ProgressFunc
			if !quiet {
				progress = func(msg string) {
					fmt.Println(msg)
				}
			}
			autoUpdate(version, progress)
			if !quiet {
				fmt.Printf("\n%s\n", brandSignature())
			}
		},
	}
	root.PersistentFlags().StringVar(&cfgFile, "config", "", "config file path")
	root.PersistentFlags().BoolVarP(&verbose, "verbose", "v", false, "verbose output")
	root.PersistentFlags().BoolVarP(&quiet, "quiet", "q", false, "suppress non-error output")

	root.AddCommand(newRunCmd(version))
	root.AddCommand(newRestoreCmd())
	root.AddCommand(newListCmd())
	root.AddCommand(newStatusCmd())
	root.AddCommand(newInitCmd())
	root.AddCommand(newVersionCmd(version))

	return root
}

// Execute runs the root command.
func Execute(version string) {
	root := NewRootCmd(version)
	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}
