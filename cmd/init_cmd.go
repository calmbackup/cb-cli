package cmd

import (
	"bufio"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/calmbackup/cb-cli/internal/config"
	"github.com/spf13/cobra"
)

func newInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "init",
		Short: "Initialize a new calmbackup configuration",
		Long:  "Interactive setup wizard. Run with sudo for system-wide config in /etc/calmbackup/.",
		RunE: func(cmd *cobra.Command, args []string) error {
			reader := bufio.NewReader(os.Stdin)
			configDir := config.ConfigDir()
			configPath := filepath.Join(configDir, "calmbackup.yaml")
			recoveryPath := filepath.Join(configDir, "calmbackup-recovery-key.txt")

			fmt.Println("CalmBackup Setup")
			fmt.Println("================")
			fmt.Println()
			fmt.Printf("Config will be written to: %s\n\n", configPath)

			// API Key
			fmt.Print("API Key: ")
			apiKey, _ := reader.ReadString('\n')
			apiKey = strings.TrimSpace(apiKey)
			if apiKey == "" {
				return fmt.Errorf("API key is required")
			}

			// Database driver
			fmt.Print("Database driver (mysql/pgsql/sqlite): ")
			driver, _ := reader.ReadString('\n')
			driver = strings.TrimSpace(driver)
			if driver != "mysql" && driver != "pgsql" && driver != "sqlite" {
				return fmt.Errorf("invalid driver: %s (must be mysql, pgsql, or sqlite)", driver)
			}

			var dbConfig string
			switch driver {
			case "mysql", "pgsql":
				fmt.Print("Database host [127.0.0.1]: ")
				host, _ := reader.ReadString('\n')
				host = strings.TrimSpace(host)
				if host == "" {
					host = "127.0.0.1"
				}

				defaultPort := "3306"
				if driver == "pgsql" {
					defaultPort = "5432"
				}
				fmt.Printf("Database port [%s]: ", defaultPort)
				port, _ := reader.ReadString('\n')
				port = strings.TrimSpace(port)
				if port == "" {
					port = defaultPort
				}

				fmt.Print("Database name: ")
				dbName, _ := reader.ReadString('\n')
				dbName = strings.TrimSpace(dbName)

				fmt.Print("Database user: ")
				dbUser, _ := reader.ReadString('\n')
				dbUser = strings.TrimSpace(dbUser)

				fmt.Print("Database password: ")
				dbPass, _ := reader.ReadString('\n')
				dbPass = strings.TrimSpace(dbPass)

				dbConfig = fmt.Sprintf(`database:
  driver: %s
  host: "%s"
  port: %s
  database: "%s"
  username: "%s"
  password: "%s"`, driver, host, port, dbName, dbUser, dbPass)

			case "sqlite":
				fmt.Print("Database path: ")
				dbPath, _ := reader.ReadString('\n')
				dbPath = strings.TrimSpace(dbPath)

				dbConfig = fmt.Sprintf(`database:
  driver: sqlite
  path: "%s"`, dbPath)
			}

			// Generate encryption key
			keyBytes := make([]byte, 32)
			if _, err := rand.Read(keyBytes); err != nil {
				return fmt.Errorf("failed to generate encryption key: %w", err)
			}
			encryptionKey := hex.EncodeToString(keyBytes)

			// Write config
			cfgContent := fmt.Sprintf(`api_key: "%s"
encryption_key: "%s"

%s

directories: []

local_path: "%s"
local_retention_days: 7
`, apiKey, encryptionKey, dbConfig, config.LocalPath())

			if err := os.MkdirAll(configDir, 0755); err != nil {
				return fmt.Errorf("failed to create config directory %s: %w", configDir, err)
			}

			if err := os.MkdirAll(config.LocalPath(), 0750); err != nil {
				fmt.Fprintf(os.Stderr, "Warning: could not create backup directory %s: %v\n", config.LocalPath(), err)
			}

			if err := os.WriteFile(configPath, []byte(cfgContent), 0600); err != nil {
				return fmt.Errorf("failed to write config: %w", err)
			}

			// Write recovery key file
			if err := os.WriteFile(recoveryPath, []byte(encryptionKey+"\n"), 0600); err != nil {
				return fmt.Errorf("failed to write recovery key: %w", err)
			}

			fmt.Println()
			fmt.Printf("Config written to:       %s\n", configPath)
			fmt.Printf("Recovery key written to: %s\n", recoveryPath)
			fmt.Println()
			fmt.Println("IMPORTANT: Store the recovery key in a safe place.")
			fmt.Println("Without it, your backups cannot be decrypted.")
			fmt.Println()
			fmt.Println("Run your first backup:")
			fmt.Println("  calmbackup run")
			fmt.Println()
			fmt.Println("Check status:")
			fmt.Println("  calmbackup status")

			return nil
		},
	}
}
