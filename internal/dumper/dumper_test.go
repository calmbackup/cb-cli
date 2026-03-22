package dumper

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestFactory_MySQL(t *testing.T) {
	d, err := NewDumper(DBConfig{
		Driver:   "mysql",
		Host:     "localhost",
		Port:     3306,
		Username: "root",
		Password: "secret",
		Database: "mydb",
	})
	if err != nil {
		t.Fatalf("NewDumper: %v", err)
	}
	if _, ok := d.(*MySQLDumper); !ok {
		t.Errorf("expected *MySQLDumper, got %T", d)
	}
}

func TestFactory_Postgres(t *testing.T) {
	d, err := NewDumper(DBConfig{
		Driver:   "pgsql",
		Host:     "localhost",
		Port:     5432,
		Username: "postgres",
		Password: "secret",
		Database: "mydb",
	})
	if err != nil {
		t.Fatalf("NewDumper: %v", err)
	}
	if _, ok := d.(*PostgresDumper); !ok {
		t.Errorf("expected *PostgresDumper, got %T", d)
	}
}

func TestFactory_PostgresAlias(t *testing.T) {
	d, err := NewDumper(DBConfig{Driver: "postgres", Host: "h", Port: 5432, Username: "u", Password: "p", Database: "d"})
	if err != nil {
		t.Fatalf("NewDumper: %v", err)
	}
	if _, ok := d.(*PostgresDumper); !ok {
		t.Errorf("expected *PostgresDumper, got %T", d)
	}
}

func TestFactory_SQLite(t *testing.T) {
	d, err := NewDumper(DBConfig{
		Driver: "sqlite",
		Path:   "/tmp/test.db",
	})
	if err != nil {
		t.Fatalf("NewDumper: %v", err)
	}
	if _, ok := d.(*SQLiteDumper); !ok {
		t.Errorf("expected *SQLiteDumper, got %T", d)
	}
}

func TestFactory_UnknownDriver(t *testing.T) {
	_, err := NewDumper(DBConfig{Driver: "oracle"})
	if err == nil {
		t.Error("expected error for unknown driver")
	}
}

func TestFilename_MySQL(t *testing.T) {
	d := &MySQLDumper{}
	if d.Filename() != "database.sql" {
		t.Errorf("expected 'database.sql', got %q", d.Filename())
	}
}

func TestFilename_Postgres(t *testing.T) {
	d := &PostgresDumper{}
	if d.Filename() != "database.pgdump" {
		t.Errorf("expected 'database.pgdump', got %q", d.Filename())
	}
}

func TestFilename_SQLite(t *testing.T) {
	d := &SQLiteDumper{}
	if d.Filename() != "database.sqlite" {
		t.Errorf("expected 'database.sqlite', got %q", d.Filename())
	}
}

// --- MySQL command construction tests ---

func TestMySQL_DumpCmd(t *testing.T) {
	d := &MySQLDumper{config: DBConfig{
		Host:     "db.example.com",
		Port:     3307,
		Username: "admin",
		Password: "p@ss",
		Database: "production",
	}}
	cmd := d.dumpCmd("/tmp/out.sql")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "mysqldump") {
		t.Error("expected mysqldump command")
	}
	if !strings.Contains(args, "--single-transaction") {
		t.Error("expected --single-transaction")
	}
	if !strings.Contains(args, "--routines") {
		t.Error("expected --routines")
	}
	if !strings.Contains(args, "--triggers") {
		t.Error("expected --triggers")
	}
	if !strings.Contains(args, "-h") || !strings.Contains(args, "db.example.com") {
		t.Error("expected host flag")
	}
	if !strings.Contains(args, "-P") || !strings.Contains(args, "3307") {
		t.Error("expected port flag")
	}
	if !strings.Contains(args, "-u") || !strings.Contains(args, "admin") {
		t.Error("expected user flag")
	}
	if !strings.Contains(args, "-pp@ss") {
		t.Error("expected password flag")
	}
	if !strings.Contains(args, "production") {
		t.Error("expected database name")
	}
}

func TestMySQL_RestoreCmd(t *testing.T) {
	d := &MySQLDumper{config: DBConfig{
		Host:     "localhost",
		Port:     3306,
		Username: "root",
		Password: "secret",
		Database: "mydb",
	}}
	cmd := d.restoreCmd("/tmp/dump.sql")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "mysql") {
		t.Error("expected mysql command")
	}
	if !strings.Contains(args, "-h") || !strings.Contains(args, "localhost") {
		t.Error("expected host flag")
	}
	if !strings.Contains(args, "-u") || !strings.Contains(args, "root") {
		t.Error("expected user flag")
	}
	if !strings.Contains(args, "mydb") {
		t.Error("expected database name")
	}
}

// --- PostgreSQL command construction tests ---

func TestPostgres_DumpCmd(t *testing.T) {
	d := &PostgresDumper{config: DBConfig{
		Host:     "pg.example.com",
		Port:     5433,
		Username: "pguser",
		Password: "pgpass",
		Database: "pgdb",
	}}
	cmd := d.dumpCmd("/tmp/out.pgdump")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "pg_dump") {
		t.Error("expected pg_dump command")
	}
	if !strings.Contains(args, "--format=custom") {
		t.Error("expected --format=custom")
	}
	if !strings.Contains(args, "-h") || !strings.Contains(args, "pg.example.com") {
		t.Error("expected host flag")
	}
	if !strings.Contains(args, "-p") || !strings.Contains(args, "5433") {
		t.Error("expected port flag")
	}
	if !strings.Contains(args, "-U") || !strings.Contains(args, "pguser") {
		t.Error("expected user flag")
	}
	if !strings.Contains(args, "pgdb") {
		t.Error("expected database name")
	}
	if !strings.Contains(args, "-f") || !strings.Contains(args, "/tmp/out.pgdump") {
		t.Error("expected output flag")
	}

	// Check PGPASSWORD env
	hasPassword := false
	for _, env := range cmd.Env {
		if strings.HasPrefix(env, "PGPASSWORD=pgpass") {
			hasPassword = true
		}
	}
	if !hasPassword {
		t.Error("expected PGPASSWORD in env")
	}
}

func TestPostgres_RestoreCmd(t *testing.T) {
	d := &PostgresDumper{config: DBConfig{
		Host:     "localhost",
		Port:     5432,
		Username: "postgres",
		Password: "secret",
		Database: "mydb",
	}}
	cmd := d.restoreCmd("/tmp/dump.pgdump")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "pg_restore") {
		t.Error("expected pg_restore command")
	}
	if !strings.Contains(args, "--clean") {
		t.Error("expected --clean")
	}
	if !strings.Contains(args, "--if-exists") {
		t.Error("expected --if-exists")
	}
	if !strings.Contains(args, "-d") || !strings.Contains(args, "mydb") {
		t.Error("expected database flag")
	}

	hasPassword := false
	for _, env := range cmd.Env {
		if strings.HasPrefix(env, "PGPASSWORD=secret") {
			hasPassword = true
		}
	}
	if !hasPassword {
		t.Error("expected PGPASSWORD in env")
	}
}

func TestPostgres_VerifyCmd(t *testing.T) {
	d := &PostgresDumper{config: DBConfig{}}
	cmd := d.verifyCmd("/tmp/dump.pgdump")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "pg_restore") {
		t.Error("expected pg_restore command")
	}
	if !strings.Contains(args, "--list") {
		t.Error("expected --list")
	}
	if !strings.Contains(args, "/tmp/dump.pgdump") {
		t.Error("expected dump path")
	}
}

// --- SQLite round-trip test ---

func TestSQLite_RoundTrip(t *testing.T) {
	if _, err := exec.LookPath("sqlite3"); err != nil {
		t.Skip("sqlite3 not available")
	}

	tmp := t.TempDir()
	dbPath := filepath.Join(tmp, "test.db")

	// Create a test database
	cmd := exec.Command("sqlite3", dbPath, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users VALUES (1, 'Alice'); INSERT INTO users VALUES (2, 'Bob');")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("create db: %v: %s", err, out)
	}

	d, err := NewDumper(DBConfig{Driver: "sqlite", Path: dbPath})
	if err != nil {
		t.Fatal(err)
	}

	// Dump
	dumpPath := filepath.Join(tmp, d.Filename())
	if err := d.Dump(dumpPath); err != nil {
		t.Fatalf("Dump: %v", err)
	}

	// Verify dump exists and is not empty
	info, err := os.Stat(dumpPath)
	if err != nil {
		t.Fatalf("stat dump: %v", err)
	}
	if info.Size() == 0 {
		t.Error("dump file is empty")
	}

	// Verify
	ok, err := d.Verify(dumpPath)
	if err != nil {
		t.Fatalf("Verify: %v", err)
	}
	if !ok {
		t.Error("verify returned false")
	}

	// Corrupt original db to test restore
	if err := os.WriteFile(dbPath, []byte("corrupted"), 0o644); err != nil {
		t.Fatal(err)
	}

	// Restore
	if err := d.Restore(dumpPath); err != nil {
		t.Fatalf("Restore: %v", err)
	}

	// Verify restored data
	out, err := exec.Command("sqlite3", dbPath, "SELECT name FROM users ORDER BY id;").Output()
	if err != nil {
		t.Fatalf("query restored db: %v", err)
	}
	lines := strings.TrimSpace(string(out))
	if lines != "Alice\nBob" {
		t.Errorf("unexpected restored data: %q", lines)
	}
}

func TestSQLite_DumpCmd(t *testing.T) {
	d := &SQLiteDumper{config: DBConfig{Path: "/data/app.db"}}
	cmd := d.dumpCmd("/tmp/backup.sqlite")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "sqlite3") {
		t.Error("expected sqlite3 command")
	}
	if !strings.Contains(args, "/data/app.db") {
		t.Error("expected database path")
	}
	if !strings.Contains(args, ".backup") {
		t.Error("expected .backup command")
	}
}

func TestSQLite_VerifyCmd(t *testing.T) {
	d := &SQLiteDumper{config: DBConfig{}}
	cmd := d.verifyCmd("/tmp/backup.sqlite")

	args := strings.Join(cmd.Args, " ")
	if !strings.Contains(args, "sqlite3") {
		t.Error("expected sqlite3 command")
	}
	if !strings.Contains(args, "/tmp/backup.sqlite") {
		t.Error("expected dump path")
	}
	if !strings.Contains(args, "PRAGMA integrity_check") {
		t.Error("expected integrity_check pragma")
	}
}
