package cmd

import (
	"os"

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
