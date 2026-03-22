package cmd

import (
	"bufio"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"
)

func newInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "init",
		Short: "Initialize a new calmbackup configuration",
		RunE: func(cmd *cobra.Command, args []string) error {
			reader := bufio.NewReader(os.Stdin)

			fmt.Println("CalmBackup Setup")
			fmt.Println("================")
			fmt.Println()

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
  port: "%s"
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
			config := fmt.Sprintf(`api_key: "%s"
encryption_key: "%s"
%s
directories: []
local_path: "backups"
local_retention_days: 7
`, apiKey, encryptionKey, dbConfig)

			if err := os.WriteFile("calmbackup.yaml", []byte(config), 0600); err != nil {
				return fmt.Errorf("failed to write config: %w", err)
			}

			// Write recovery key file
			if err := os.WriteFile("calmbackup-recovery-key.txt", []byte(encryptionKey+"\n"), 0600); err != nil {
				return fmt.Errorf("failed to write recovery key: %w", err)
			}

			fmt.Println()
			fmt.Println("Configuration written to calmbackup.yaml")
			fmt.Println("Recovery key written to calmbackup-recovery-key.txt")
			fmt.Println()
			fmt.Println("IMPORTANT:")
			fmt.Println("  1. Add both files to .gitignore:")
			fmt.Println("     echo 'calmbackup.yaml' >> .gitignore")
			fmt.Println("     echo 'calmbackup-recovery-key.txt' >> .gitignore")
			fmt.Println()
			fmt.Println("  2. Store the recovery key in a safe place.")
			fmt.Println("     Without it, your backups cannot be decrypted.")
			fmt.Println()
			fmt.Println("  3. To schedule automatic backups, add a crontab entry:")
			fmt.Println("     0 2 * * * cd /path/to/project && calmbackup run")

			return nil
		},
	}
}
