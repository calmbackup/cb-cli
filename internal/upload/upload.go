package upload

import (
	"fmt"
	"io"
	"net/http"
	"os"
)

func Upload(filePath, presignedURL string) error {
	f, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("failed to open file: %w", err)
	}
	defer f.Close()

	stat, err := f.Stat()
	if err != nil {
		return fmt.Errorf("failed to stat file: %w", err)
	}

	req, err := http.NewRequest(http.MethodPut, presignedURL, f)
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/octet-stream")
	req.ContentLength = stat.Size()

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return fmt.Errorf("upload failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		return fmt.Errorf("upload failed with status %d", resp.StatusCode)
	}

	return nil
}

func Download(presignedURL, outputPath string) error {
	resp, err := http.Get(presignedURL)
	if err != nil {
		return fmt.Errorf("download failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		return fmt.Errorf("download failed with status %d", resp.StatusCode)
	}

	f, err := os.Create(outputPath)
	if err != nil {
		return fmt.Errorf("failed to create output file: %w", err)
	}

	_, copyErr := io.Copy(f, resp.Body)
	closeErr := f.Close()

	if copyErr != nil {
		os.Remove(outputPath)

		return fmt.Errorf("failed to write file: %w", copyErr)
	}

	if closeErr != nil {
		os.Remove(outputPath)

		return fmt.Errorf("failed to close file: %w", closeErr)
	}

	return nil
}
