package dumper

import "fmt"

// DBConfig holds database connection configuration.
type DBConfig struct {
	Driver   string
	Host     string
	Port     int
	Username string
	Password string
	Database string
	Path     string
}

// NewDumper creates a Dumper for the given database configuration.
func NewDumper(config DBConfig) (Dumper, error) {
	switch config.Driver {
	case "mysql":
		return &MySQLDumper{config: config}, nil
	case "pgsql", "postgres", "postgresql":
		return &PostgresDumper{config: config}, nil
	case "sqlite", "sqlite3":
		return &SQLiteDumper{config: config}, nil
	default:
		return nil, fmt.Errorf("unsupported database driver: %s", config.Driver)
	}
}
