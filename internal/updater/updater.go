package updater

import (
	"archive/tar"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

const (
	repo          = "calmbackup/cb-cli"
	binaryName    = "calmbackup"
	githubAPI     = "https://api.github.com"
	projectName   = "calmbackup"
)

type release struct {
	TagName string  `json:"tag_name"`
	Assets  []asset `json:"assets"`
}

type asset struct {
	Name               string `json:"name"`
	BrowserDownloadURL string `json:"browser_download_url"`
}

// Check compares the current version against the latest GitHub release.
// Returns the latest version tag and true if an update is available.
func Check(currentVersion string) (string, bool, error) {
	url := fmt.Sprintf("%s/repos/%s/releases/latest", githubAPI, repo)

	resp, err := http.Get(url)
	if err != nil {
		return "", false, fmt.Errorf("failed to check for updates: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", false, fmt.Errorf("GitHub API returned status %d", resp.StatusCode)
	}

	var rel release
	if err := json.NewDecoder(resp.Body).Decode(&rel); err != nil {
		return "", false, fmt.Errorf("failed to parse release info: %w", err)
	}

	latest := strings.TrimPrefix(rel.TagName, "v")
	current := strings.TrimPrefix(currentVersion, "v")

	if latest == "" || latest == current || currentVersion == "dev" {
		return rel.TagName, false, nil
	}

	return rel.TagName, true, nil
}

// Update downloads the latest release and replaces the current binary.
func Update(currentVersion string) (string, error) {
	latestTag, needsUpdate, err := Check(currentVersion)
	if err != nil {
		return "", err
	}

	if !needsUpdate {
		return latestTag, nil
	}

	arch := runtime.GOARCH
	osName := runtime.GOOS

	// Find the right tarball from the release assets
	tarballName := findTarball(latestTag, osName, arch)

	downloadURL := fmt.Sprintf(
		"https://github.com/%s/releases/download/%s/%s",
		repo, latestTag, tarballName,
	)

	// Download to temp file
	tmpDir, err := os.MkdirTemp("", "calmbackup-update-*")
	if err != nil {
		return "", fmt.Errorf("failed to create temp dir: %w", err)
	}
	defer os.RemoveAll(tmpDir)

	tarballPath := filepath.Join(tmpDir, tarballName)
	if err := downloadFile(downloadURL, tarballPath); err != nil {
		return "", fmt.Errorf("failed to download update: %w", err)
	}

	// Extract binary from tarball
	newBinaryPath := filepath.Join(tmpDir, binaryName)
	if err := extractBinary(tarballPath, newBinaryPath); err != nil {
		return "", fmt.Errorf("failed to extract update: %w", err)
	}

	// Replace current binary
	currentBinary, err := os.Executable()
	if err != nil {
		return "", fmt.Errorf("failed to find current binary: %w", err)
	}

	currentBinary, err = filepath.EvalSymlinks(currentBinary)
	if err != nil {
		return "", fmt.Errorf("failed to resolve binary path: %w", err)
	}

	if err := replaceBinary(newBinaryPath, currentBinary); err != nil {
		return "", fmt.Errorf("failed to replace binary: %w", err)
	}

	return latestTag, nil
}

func findTarball(tag, osName, arch string) string {
	version := strings.TrimPrefix(tag, "v")

	// Try current naming convention first, then legacy
	candidates := []string{
		fmt.Sprintf("%s_%s_%s_%s.tar.gz", projectName, version, osName, arch),
		fmt.Sprintf("cb-cli_%s_%s_%s.tar.gz", version, osName, arch),
	}

	return candidates[0]
}

func downloadFile(url, dest string) error {
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download returned status %d", resp.StatusCode)
	}

	out, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)

	return err
}

func extractBinary(tarballPath, destPath string) error {
	f, err := os.Open(tarballPath)
	if err != nil {
		return err
	}
	defer f.Close()

	gz, err := gzip.NewReader(f)
	if err != nil {
		return err
	}
	defer gz.Close()

	tr := tar.NewReader(gz)

	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}

		if err != nil {
			return err
		}

		if filepath.Base(hdr.Name) == binaryName && hdr.Typeflag == tar.TypeReg {
			out, err := os.OpenFile(destPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
			if err != nil {
				return err
			}
			defer out.Close()

			if _, err := io.Copy(out, tr); err != nil {
				return err
			}

			return nil
		}
	}

	return fmt.Errorf("binary %q not found in archive", binaryName)
}

func replaceBinary(newPath, currentPath string) error {
	// Atomic replace: rename new binary over the current one.
	// On Linux, renaming over a running binary works fine because
	// the kernel keeps the old inode alive until the process exits.

	// First try direct rename (works if same filesystem)
	if err := os.Rename(newPath, currentPath); err == nil {
		return nil
	}

	// Cross-filesystem: copy then remove
	src, err := os.Open(newPath)
	if err != nil {
		return err
	}
	defer src.Close()

	// Write to a temp file next to the target, then rename
	tmpPath := currentPath + ".new"
	dst, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
	if err != nil {
		return err
	}

	if _, err := io.Copy(dst, src); err != nil {
		dst.Close()
		os.Remove(tmpPath)

		return err
	}

	dst.Close()

	if err := os.Rename(tmpPath, currentPath); err != nil {
		os.Remove(tmpPath)

		return err
	}

	return nil
}
