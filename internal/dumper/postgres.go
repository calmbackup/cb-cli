package dumper

import (
	"fmt"
	"os"
	"os/exec"
)

// PostgresDumper handles PostgreSQL database dump, verify, and restore.
type PostgresDumper struct {
	config DBConfig
}

func (d *PostgresDumper) Filename() string {
	return "database.pgdump"
}

func (d *PostgresDumper) dumpCmd(outputPath string) *exec.Cmd {
	cmd := exec.Command("pg_dump",
		"--format=custom",
		"-h", d.config.Host,
		"-p", fmt.Sprintf("%d", d.config.Port),
		"-U", d.config.Username,
		d.config.Database,
		"-f", outputPath,
	)
	cmd.Env = append(os.Environ(), fmt.Sprintf("PGPASSWORD=%s", d.config.Password))

	return cmd
}

func (d *PostgresDumper) Dump(outputPath string) error {
	cmd := d.dumpCmd(outputPath)
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("pg_dump: %w", err)
	}

	return nil
}

func (d *PostgresDumper) verifyCmd(dumpPath string) *exec.Cmd {
	return exec.Command("pg_restore", "--list", dumpPath)
}

func (d *PostgresDumper) Verify(dumpPath string) (bool, error) {
	cmd := d.verifyCmd(dumpPath)

	if err := cmd.Run(); err != nil {
		return false, nil
	}

	return true, nil
}

func (d *PostgresDumper) restoreCmd(dumpPath string) *exec.Cmd {
	cmd := exec.Command("pg_restore",
		"--clean",
		"--if-exists",
		"-h", d.config.Host,
		"-p", fmt.Sprintf("%d", d.config.Port),
		"-U", d.config.Username,
		"-d", d.config.Database,
		dumpPath,
	)
	cmd.Env = append(os.Environ(), fmt.Sprintf("PGPASSWORD=%s", d.config.Password))

	return cmd
}

func (d *PostgresDumper) Restore(dumpPath string) error {
	cmd := d.restoreCmd(dumpPath)
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("pg_restore: %w", err)
	}

	return nil
}
