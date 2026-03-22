package dumper

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

// MySQLDumper handles MySQL database dump, verify, and restore.
type MySQLDumper struct {
	config DBConfig
}

func (d *MySQLDumper) Filename() string {
	return "database.sql"
}

func (d *MySQLDumper) dumpCmd(outputPath string) *exec.Cmd {
	return exec.Command("mysqldump",
		"--single-transaction",
		"--routines",
		"--triggers",
		"-h", d.config.Host,
		"-P", fmt.Sprintf("%d", d.config.Port),
		"-u", d.config.Username,
		fmt.Sprintf("-p%s", d.config.Password),
		d.config.Database,
	)
}

func (d *MySQLDumper) Dump(outputPath string) error {
	cmd := d.dumpCmd(outputPath)

	outFile, err := os.Create(outputPath)
	if err != nil {
		return fmt.Errorf("create output file: %w", err)
	}
	defer outFile.Close()

	cmd.Stdout = outFile
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("mysqldump: %w", err)
	}

	return nil
}

func (d *MySQLDumper) Verify(dumpPath string) (bool, error) {
	data, err := os.ReadFile(dumpPath)
	if err != nil {
		return false, fmt.Errorf("read dump: %w", err)
	}

	return strings.Contains(string(data), "-- Dump completed"), nil
}

func (d *MySQLDumper) restoreCmd(dumpPath string) *exec.Cmd {
	return exec.Command("mysql",
		"-h", d.config.Host,
		"-P", fmt.Sprintf("%d", d.config.Port),
		"-u", d.config.Username,
		fmt.Sprintf("-p%s", d.config.Password),
		d.config.Database,
	)
}

func (d *MySQLDumper) Restore(dumpPath string) error {
	cmd := d.restoreCmd(dumpPath)

	inFile, err := os.Open(dumpPath)
	if err != nil {
		return fmt.Errorf("open dump: %w", err)
	}
	defer inFile.Close()

	cmd.Stdin = inFile
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("mysql restore: %w", err)
	}

	return nil
}
