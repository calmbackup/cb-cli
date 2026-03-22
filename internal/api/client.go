package api

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
)

type Client struct {
	apiKey     string
	baseURL    string
	version    string
	httpClient *http.Client
}

type UploadURLResponse struct {
	BackupID  string `json:"backup_id"`
	UploadURL string `json:"upload_url"`
	ExpiresIn int    `json:"expires_in"`
}

type Backup struct {
	ID             string `json:"id"`
	Filename       string `json:"filename"`
	Size           int64  `json:"size"`
	Checksum       string `json:"checksum"`
	DBDriver       string `json:"db_driver"`
	Status         string `json:"status"`
	PackageVersion string `json:"package_version"`
	CreatedAt      string `json:"created_at"`
	DownloadURL    string `json:"download_url,omitempty"`
	DownloadExpiry int    `json:"download_expires_in,omitempty"`
}

type ListBackupsResponse struct {
	Data []Backup `json:"data"`
	Meta struct {
		CurrentPage int `json:"current_page"`
		LastPage    int `json:"last_page"`
		Total       int `json:"total"`
	} `json:"meta"`
}

type AccountInfo struct {
	BackupCount      int    `json:"backup_count"`
	StorageUsedBytes int64  `json:"storage_used_bytes"`
	LastBackupAt     string `json:"last_backup_at"`
}

func NewClient(apiKey, baseURL, version string) *Client {
	return &Client{
		apiKey:     apiKey,
		baseURL:    baseURL,
		version:    version,
		httpClient: &http.Client{},
	}
}

func (c *Client) do(method, path string, body interface{}) ([]byte, error) {
	url := c.baseURL + path

	var reqBody io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal request body: %w", err)
		}

		reqBody = bytes.NewReader(data)
	}

	req, err := http.NewRequest(method, url, reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", "Bearer "+c.apiKey)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	req.Header.Set("X-Backup-Version", c.version)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode >= 400 {
		return nil, parseAPIError(resp.StatusCode, respBody)
	}

	return respBody, nil
}

func (c *Client) RequestUploadURL(filename string, size int64, checksum, dbDriver string) (*UploadURLResponse, error) {
	payload := map[string]interface{}{
		"filename":  filename,
		"size":      size,
		"checksum":  checksum,
		"db_driver": dbDriver,
	}

	body, err := c.do(http.MethodPost, "/upload-url", payload)
	if err != nil {
		return nil, err
	}

	var resp UploadURLResponse
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &resp, nil
}

func (c *Client) ConfirmBackup(backupID string, size int64, checksum string) error {
	payload := map[string]interface{}{
		"size":     size,
		"checksum": checksum,
	}

	_, err := c.do(http.MethodPost, "/backups/"+backupID+"/confirm", payload)

	return err
}

func (c *Client) ListBackups(page, perPage int) (*ListBackupsResponse, error) {
	path := fmt.Sprintf("/backups?page=%d&per_page=%d", page, perPage)

	body, err := c.do(http.MethodGet, path, nil)
	if err != nil {
		return nil, err
	}

	var resp ListBackupsResponse
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &resp, nil
}

func (c *Client) GetBackup(backupID string) (*Backup, error) {
	body, err := c.do(http.MethodGet, "/backups/"+backupID, nil)
	if err != nil {
		return nil, err
	}

	var backup Backup
	if err := json.Unmarshal(body, &backup); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &backup, nil
}

func (c *Client) DeleteBackup(backupID string) error {
	_, err := c.do(http.MethodDelete, "/backups/"+backupID, nil)

	return err
}

func (c *Client) GetAccount() (*AccountInfo, error) {
	body, err := c.do(http.MethodGet, "/account", nil)
	if err != nil {
		return nil, err
	}

	var info AccountInfo
	if err := json.Unmarshal(body, &info); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &info, nil
}
