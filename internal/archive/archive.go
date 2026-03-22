package archive

import (
	"archive/tar"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// Create produces a .tar.gz archive containing the dump file at the root
// and each directory as a top-level folder named by its basename.
func Create(dumpPath string, directories []string, outputPath string) error {
	outFile, err := os.Create(outputPath)
	if err != nil {
		return fmt.Errorf("create output file: %w", err)
	}
	defer outFile.Close()

	gw := gzip.NewWriter(outFile)
	defer gw.Close()

	tw := tar.NewWriter(gw)
	defer tw.Close()

	// Add the dump file at the root of the archive
	if err := addFile(tw, dumpPath, filepath.Base(dumpPath)); err != nil {
		return fmt.Errorf("add dump file: %w", err)
	}

	// Add each directory
	for _, dir := range directories {
		baseName := filepath.Base(dir)
		if err := addDirectory(tw, dir, baseName); err != nil {
			return fmt.Errorf("add directory %s: %w", dir, err)
		}
	}

	return nil
}

// Extract extracts a .tar.gz archive to outputDir and returns the list of extracted file paths.
func Extract(archivePath string, outputDir string) ([]string, error) {
	f, err := os.Open(archivePath)
	if err != nil {
		return nil, fmt.Errorf("open archive: %w", err)
	}
	defer f.Close()

	gr, err := gzip.NewReader(f)
	if err != nil {
		return nil, fmt.Errorf("gzip reader: %w", err)
	}
	defer gr.Close()

	tr := tar.NewReader(gr)
	var extracted []string

	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, fmt.Errorf("tar next: %w", err)
		}

		// Sanitize path to prevent directory traversal
		name := hdr.Name
		if strings.Contains(name, "..") {
			continue
		}

		target := filepath.Join(outputDir, name)

		switch hdr.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, 0o755); err != nil {
				return nil, fmt.Errorf("mkdir %s: %w", target, err)
			}
			extracted = append(extracted, target)
		case tar.TypeReg:
			if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
				return nil, fmt.Errorf("mkdir parent %s: %w", target, err)
			}
			outFile, err := os.OpenFile(target, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, os.FileMode(hdr.Mode))
			if err != nil {
				return nil, fmt.Errorf("create file %s: %w", target, err)
			}
			if _, err := io.Copy(outFile, tr); err != nil {
				outFile.Close()
				return nil, fmt.Errorf("write file %s: %w", target, err)
			}
			outFile.Close()
			extracted = append(extracted, target)
		}
	}

	return extracted, nil
}

func addFile(tw *tar.Writer, filePath, archiveName string) error {
	info, err := os.Stat(filePath)
	if err != nil {
		return err
	}

	hdr, err := tar.FileInfoHeader(info, "")
	if err != nil {
		return err
	}
	hdr.Name = archiveName

	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}

	f, err := os.Open(filePath)
	if err != nil {
		return err
	}
	defer f.Close()

	_, err = io.Copy(tw, f)
	return err
}

func addDirectory(tw *tar.Writer, dirPath, prefix string) error {
	return filepath.Walk(dirPath, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(dirPath, path)
		if err != nil {
			return err
		}

		archiveName := filepath.Join(prefix, rel)
		// Use forward slashes in tar
		archiveName = filepath.ToSlash(archiveName)

		if info.IsDir() {
			hdr := &tar.Header{
				Name:     archiveName + "/",
				Mode:     int64(info.Mode()),
				Typeflag: tar.TypeDir,
				ModTime:  info.ModTime(),
			}
			return tw.WriteHeader(hdr)
		}

		hdr, err := tar.FileInfoHeader(info, "")
		if err != nil {
			return err
		}
		hdr.Name = archiveName

		if err := tw.WriteHeader(hdr); err != nil {
			return err
		}

		f, err := os.Open(path)
		if err != nil {
			return err
		}
		defer f.Close()

		_, err = io.Copy(tw, f)
		return err
	})
}
