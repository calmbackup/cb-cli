package dumper

// Dumper defines the interface for database dump, verify, and restore operations.
type Dumper interface {
	Dump(outputPath string) error
	Verify(dumpPath string) (bool, error)
	Restore(dumpPath string) error
	Filename() string
}
