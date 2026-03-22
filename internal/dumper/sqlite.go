package dumper

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
)

// SQLiteDumper handles SQLite database dump, verify, and restore.
type SQLiteDumper struct {
	config DBConfig
}

func (d *SQLiteDumper) Filename() string {
	return "database.sqlite"
}

func (d *SQLiteDumper) dumpCmd(outputPath string) *exec.Cmd {
	return exec.Command("sqlite3", d.config.Path, fmt.Sprintf(".backup '%s'", outputPath))
}

func (d *SQLiteDumper) Dump(outputPath string) error {
	// Try sqlite3 .backup first
	if _, err := exec.LookPath("sqlite3"); err == nil {
		cmd := d.dumpCmd(outputPath)
		cmd.Stderr = os.Stderr

		if err := cmd.Run(); err == nil {
			return nil
		}
	}

	// Fallback: file copy
	return copyFile(d.config.Path, outputPath)
}

func (d *SQLiteDumper) verifyCmd(dumpPath string) *exec.Cmd {
	return exec.Command("sqlite3", dumpPath, "PRAGMA integrity_check")
}

func (d *SQLiteDumper) Verify(dumpPath string) (bool, error) {
	cmd := d.verifyCmd(dumpPath)

	out, err := cmd.Output()
	if err != nil {
		return false, fmt.Errorf("integrity check: %w", err)
	}

	return strings.TrimSpace(string(out)) == "ok", nil
}

func (d *SQLiteDumper) Restore(dumpPath string) error {
	return copyFile(dumpPath, d.config.Path)
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return fmt.Errorf("open source: %w", err)
	}
	defer in.Close()

	out, err := os.Create(dst)
	if err != nil {
		return fmt.Errorf("create destination: %w", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, in); err != nil {
		return fmt.Errorf("copy: %w", err)
	}

	return nil
}
