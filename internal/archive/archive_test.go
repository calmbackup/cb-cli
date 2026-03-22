package archive

import (
	"archive/tar"
	"compress/gzip"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

// listTarGzEntries opens a .tar.gz and returns sorted entry names.
func listTarGzEntries(t *testing.T, archivePath string) []string {
	t.Helper()
	f, err := os.Open(archivePath)
	if err != nil {
		t.Fatalf("open archive: %v", err)
	}
	defer f.Close()

	gr, err := gzip.NewReader(f)
	if err != nil {
		t.Fatalf("gzip reader: %v", err)
	}
	defer gr.Close()

	tr := tar.NewReader(gr)
	var names []string
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("tar next: %v", err)
		}
		names = append(names, hdr.Name)
	}
	sort.Strings(names)
	return names
}

// writeFile is a test helper to create a file with content.
func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestCreate_SingleDumpFile(t *testing.T) {
	tmp := t.TempDir()

	dumpPath := filepath.Join(tmp, "database.sql")
	writeFile(t, dumpPath, "-- SQL dump content")

	outPath := filepath.Join(tmp, "backup.tar.gz")
	if err := Create(dumpPath, nil, outPath); err != nil {
		t.Fatalf("Create: %v", err)
	}

	entries := listTarGzEntries(t, outPath)
	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d: %v", len(entries), entries)
	}
	if entries[0] != "database.sql" {
		t.Errorf("expected 'database.sql', got %q", entries[0])
	}
}

func TestCreate_DumpWithDirectories(t *testing.T) {
	tmp := t.TempDir()

	dumpPath := filepath.Join(tmp, "database.sql")
	writeFile(t, dumpPath, "-- SQL dump")

	// Create directory structure
	dir1 := filepath.Join(tmp, "uploads")
	writeFile(t, filepath.Join(dir1, "photo.jpg"), "jpeg-data")
	writeFile(t, filepath.Join(dir1, "sub", "doc.pdf"), "pdf-data")

	dir2 := filepath.Join(tmp, "config")
	writeFile(t, filepath.Join(dir2, "app.yaml"), "key: value")

	outPath := filepath.Join(tmp, "backup.tar.gz")
	if err := Create(dumpPath, []string{dir1, dir2}, outPath); err != nil {
		t.Fatalf("Create: %v", err)
	}

	entries := listTarGzEntries(t, outPath)

	// Check dump file is at root
	found := false
	for _, e := range entries {
		if e == "database.sql" {
			found = true
		}
	}
	if !found {
		t.Error("dump file 'database.sql' not found at root")
	}

	// Check directory entries use basename as prefix
	expectPrefixes := []string{"uploads/", "config/"}
	for _, prefix := range expectPrefixes {
		hasPrefix := false
		for _, e := range entries {
			if strings.HasPrefix(e, prefix) {
				hasPrefix = true
				break
			}
		}
		if !hasPrefix {
			t.Errorf("no entries with prefix %q in %v", prefix, entries)
		}
	}

	// Check nested file preserved
	hasNested := false
	for _, e := range entries {
		if e == "uploads/sub/doc.pdf" {
			hasNested = true
		}
	}
	if !hasNested {
		t.Error("nested file 'uploads/sub/doc.pdf' not found")
	}
}

func TestCreate_EmptyDirectories(t *testing.T) {
	tmp := t.TempDir()

	dumpPath := filepath.Join(tmp, "database.sql")
	writeFile(t, dumpPath, "-- SQL dump")

	emptyDir := filepath.Join(tmp, "empty")
	if err := os.MkdirAll(emptyDir, 0o755); err != nil {
		t.Fatal(err)
	}

	outPath := filepath.Join(tmp, "backup.tar.gz")
	if err := Create(dumpPath, []string{emptyDir}, outPath); err != nil {
		t.Fatalf("Create: %v", err)
	}

	entries := listTarGzEntries(t, outPath)
	// Should at least contain the dump and the empty dir entry
	hasDump := false
	hasEmptyDir := false
	for _, e := range entries {
		if e == "database.sql" {
			hasDump = true
		}
		if e == "empty/" || e == "empty" {
			hasEmptyDir = true
		}
	}
	if !hasDump {
		t.Error("dump file not found")
	}
	if !hasEmptyDir {
		t.Error("empty directory entry not found")
	}
}

func TestRoundTrip(t *testing.T) {
	tmp := t.TempDir()

	// Create source files
	dumpPath := filepath.Join(tmp, "src", "database.sql")
	writeFile(t, dumpPath, "-- Full SQL dump\nCREATE TABLE foo;")

	dir1 := filepath.Join(tmp, "src", "uploads")
	writeFile(t, filepath.Join(dir1, "a.txt"), "file-a")
	writeFile(t, filepath.Join(dir1, "nested", "b.txt"), "file-b")

	// Create archive
	archivePath := filepath.Join(tmp, "backup.tar.gz")
	if err := Create(dumpPath, []string{dir1}, archivePath); err != nil {
		t.Fatalf("Create: %v", err)
	}

	// Extract
	extractDir := filepath.Join(tmp, "extracted")
	files, err := Extract(archivePath, extractDir)
	if err != nil {
		t.Fatalf("Extract: %v", err)
	}

	if len(files) == 0 {
		t.Fatal("no files extracted")
	}

	// Verify dump content
	data, err := os.ReadFile(filepath.Join(extractDir, "database.sql"))
	if err != nil {
		t.Fatalf("read dump: %v", err)
	}
	if string(data) != "-- Full SQL dump\nCREATE TABLE foo;" {
		t.Errorf("dump content mismatch: %q", data)
	}

	// Verify directory file content
	data, err = os.ReadFile(filepath.Join(extractDir, "uploads", "a.txt"))
	if err != nil {
		t.Fatalf("read uploads/a.txt: %v", err)
	}
	if string(data) != "file-a" {
		t.Errorf("a.txt content mismatch: %q", data)
	}

	data, err = os.ReadFile(filepath.Join(extractDir, "uploads", "nested", "b.txt"))
	if err != nil {
		t.Fatalf("read uploads/nested/b.txt: %v", err)
	}
	if string(data) != "file-b" {
		t.Errorf("b.txt content mismatch: %q", data)
	}
}

func TestCreate_NestedDirectoryStructure(t *testing.T) {
	tmp := t.TempDir()

	dumpPath := filepath.Join(tmp, "db.sql")
	writeFile(t, dumpPath, "dump")

	dir := filepath.Join(tmp, "data")
	writeFile(t, filepath.Join(dir, "a", "b", "c", "deep.txt"), "deep")
	writeFile(t, filepath.Join(dir, "a", "shallow.txt"), "shallow")

	outPath := filepath.Join(tmp, "backup.tar.gz")
	if err := Create(dumpPath, []string{dir}, outPath); err != nil {
		t.Fatalf("Create: %v", err)
	}

	entries := listTarGzEntries(t, outPath)

	hasDeep := false
	hasShallow := false
	for _, e := range entries {
		if e == "data/a/b/c/deep.txt" {
			hasDeep = true
		}
		if e == "data/a/shallow.txt" {
			hasShallow = true
		}
	}
	if !hasDeep {
		t.Errorf("nested file 'data/a/b/c/deep.txt' not found in %v", entries)
	}
	if !hasShallow {
		t.Errorf("file 'data/a/shallow.txt' not found in %v", entries)
	}
}
