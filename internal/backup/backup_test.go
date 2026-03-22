package backup

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

// --- Mock implementations ---

type mockDumper struct {
	mu          sync.Mutex
	calls       []string
	dumpErr     error
	verifyErr   error
	verifyOK    bool
	restoreErr  error
	filenameFn  func() string
}

func (m *mockDumper) Dump(outputPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, "dump:"+outputPath)
	m.mu.Unlock()
	if m.dumpErr != nil {
		return m.dumpErr
	}
	// Create a fake dump file
	return os.WriteFile(outputPath, []byte("fake-dump"), 0644)
}

func (m *mockDumper) Verify(dumpPath string) (bool, error) {
	m.mu.Lock()
	m.calls = append(m.calls, "verify:"+dumpPath)
	m.mu.Unlock()
	if m.verifyErr != nil {
		return false, m.verifyErr
	}
	return m.verifyOK, nil
}

func (m *mockDumper) Restore(dumpPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, "restore:"+dumpPath)
	m.mu.Unlock()
	return m.restoreErr
}

func (m *mockDumper) Filename() string {
	if m.filenameFn != nil {
		return m.filenameFn()
	}
	return "database.sql"
}

type mockArchiver struct {
	mu         sync.Mutex
	calls      []string
	createErr  error
	extractErr error
	extractDirs []string
}

func (m *mockArchiver) Create(dumpPath string, directories []string, outputPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("create:%s:%s", dumpPath, outputPath))
	m.mu.Unlock()
	if m.createErr != nil {
		return m.createErr
	}
	return os.WriteFile(outputPath, []byte("fake-archive"), 0644)
}

func (m *mockArchiver) Extract(archivePath string, outputDir string) ([]string, error) {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("extract:%s:%s", archivePath, outputDir))
	m.mu.Unlock()
	if m.extractErr != nil {
		return nil, m.extractErr
	}
	// Create fake extracted dump file
	os.WriteFile(filepath.Join(outputDir, "database.sql"), []byte("fake-dump"), 0644)
	return m.extractDirs, nil
}

type mockEncryptor struct {
	mu         sync.Mutex
	calls      []string
	encryptErr error
	decryptErr error
}

func (m *mockEncryptor) Encrypt(inputPath, outputPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("encrypt:%s:%s", inputPath, outputPath))
	m.mu.Unlock()
	if m.encryptErr != nil {
		return m.encryptErr
	}
	data, _ := os.ReadFile(inputPath)
	return os.WriteFile(outputPath, data, 0644)
}

func (m *mockEncryptor) Decrypt(inputPath, outputPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("decrypt:%s:%s", inputPath, outputPath))
	m.mu.Unlock()
	if m.decryptErr != nil {
		return m.decryptErr
	}
	data, _ := os.ReadFile(inputPath)
	return os.WriteFile(outputPath, data, 0644)
}

type mockAPIClient struct {
	mu               sync.Mutex
	calls            []string
	requestUploadErr error
	confirmErr       error
	listBackupsResp  *ListBackupsResponse
	listBackupsErr   error
	getBackupResp    *BackupDetail
	getBackupErr     error
	uploadCounter    int
}

func (m *mockAPIClient) RequestUploadURL(filename string, size int64, checksum, dbDriver string) (*UploadURLResponse, error) {
	m.mu.Lock()
	m.calls = append(m.calls, "request-upload:"+filename)
	m.mu.Unlock()
	if m.requestUploadErr != nil {
		return nil, m.requestUploadErr
	}
	return &UploadURLResponse{
		BackupID:  "backup-123",
		UploadURL: "https://storage.example.com/upload",
	}, nil
}

func (m *mockAPIClient) ConfirmBackup(backupID string, size int64, checksum string) error {
	m.mu.Lock()
	m.calls = append(m.calls, "confirm:"+backupID)
	m.mu.Unlock()
	return m.confirmErr
}

func (m *mockAPIClient) ListBackups(page, perPage int) (*ListBackupsResponse, error) {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("list-backups:%d:%d", page, perPage))
	m.mu.Unlock()
	if m.listBackupsErr != nil {
		return nil, m.listBackupsErr
	}
	if m.listBackupsResp != nil {
		return m.listBackupsResp, nil
	}
	return &ListBackupsResponse{}, nil
}

func (m *mockAPIClient) GetBackup(backupID string) (*BackupDetail, error) {
	m.mu.Lock()
	m.calls = append(m.calls, "get-backup:"+backupID)
	m.mu.Unlock()
	if m.getBackupErr != nil {
		return nil, m.getBackupErr
	}
	return m.getBackupResp, nil
}

type mockUploader struct {
	mu          sync.Mutex
	calls       []string
	uploadErr   error
	downloadErr error
}

func (m *mockUploader) Upload(filePath, presignedURL string) error {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("upload:%s:%s", filepath.Base(filePath), presignedURL))
	m.mu.Unlock()
	return m.uploadErr
}

func (m *mockUploader) Download(presignedURL, outputPath string) error {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("download:%s:%s", presignedURL, filepath.Base(outputPath)))
	m.mu.Unlock()
	if m.downloadErr != nil {
		return m.downloadErr
	}
	return os.WriteFile(outputPath, []byte("encrypted-data"), 0644)
}

type mockPruner struct {
	mu       sync.Mutex
	calls    []string
	pruneErr error
	pruned   int
}

func (m *mockPruner) Prune(backupDir string, retentionDays int, confirmedFilenames []string) (int, error) {
	m.mu.Lock()
	m.calls = append(m.calls, fmt.Sprintf("prune:%s:%d", backupDir, retentionDays))
	m.mu.Unlock()
	if m.pruneErr != nil {
		return 0, m.pruneErr
	}
	return m.pruned, nil
}

// --- Helper ---

func newTestService(t *testing.T) (*Service, *mockDumper, *mockArchiver, *mockEncryptor, *mockAPIClient, *mockUploader, *mockPruner) {
	t.Helper()
	localPath := t.TempDir()

	dumper := &mockDumper{verifyOK: true}
	archiver := &mockArchiver{}
	encryptor := &mockEncryptor{}
	api := &mockAPIClient{
		listBackupsResp: &ListBackupsResponse{},
	}
	uploader := &mockUploader{}
	pruner := &mockPruner{}

	svc := &Service{
		Dumper:    dumper,
		Archiver:  archiver,
		Encryptor: encryptor,
		API:       api,
		Uploader:  uploader,
		Pruner:    pruner,
		Config: ServiceConfig{
			DBDriver:      "mysql",
			Directories:   []string{"/var/data", "/home/user/docs"},
			LocalPath:     localPath,
			RetentionDays: 7,
		},
	}

	return svc, dumper, archiver, encryptor, api, uploader, pruner
}

func collectProgress(fn func(ProgressFunc) Result) (Result, []string) {
	var messages []string
	var mu sync.Mutex
	result := fn(func(msg string) {
		mu.Lock()
		messages = append(messages, msg)
		mu.Unlock()
	})
	return result, messages
}

// --- Tests ---

func TestBackup_Success(t *testing.T) {
	svc, dumper, archiver, encryptor, api, uploader, pruner := newTestService(t)

	result, progress := collectProgress(func(pf ProgressFunc) Result {
		return svc.Backup(pf)
	})

	if !result.Success {
		t.Fatalf("expected success, got error: %s", result.Error)
	}
	if result.Filename == "" {
		t.Error("expected non-empty filename")
	}
	if result.Size <= 0 {
		t.Error("expected positive size")
	}
	if result.Duration <= 0 {
		t.Error("expected positive duration")
	}

	// Verify dump was called
	if len(dumper.calls) < 2 {
		t.Fatalf("expected at least 2 dumper calls (dump + verify), got %d", len(dumper.calls))
	}
	if !strings.HasPrefix(dumper.calls[0], "dump:") {
		t.Errorf("expected first call to be dump, got %s", dumper.calls[0])
	}
	if !strings.HasPrefix(dumper.calls[1], "verify:") {
		t.Errorf("expected second call to be verify, got %s", dumper.calls[1])
	}

	// Verify archive was called
	if len(archiver.calls) < 1 {
		t.Fatal("expected archiver.Create to be called")
	}
	if !strings.HasPrefix(archiver.calls[0], "create:") {
		t.Errorf("expected create call, got %s", archiver.calls[0])
	}

	// Verify encrypt was called
	if len(encryptor.calls) < 1 {
		t.Fatal("expected encryptor.Encrypt to be called")
	}

	// Verify upload was called
	if len(api.calls) < 1 {
		t.Fatal("expected API calls")
	}

	// Verify uploader was called
	if len(uploader.calls) < 1 {
		t.Fatal("expected uploader.Upload to be called")
	}

	// Verify pruner was called
	if len(pruner.calls) < 1 {
		t.Fatal("expected pruner.Prune to be called")
	}

	// Verify progress messages were sent
	if len(progress) == 0 {
		t.Error("expected progress messages")
	}

	// Verify the encrypted file was copied to local path
	files, _ := os.ReadDir(svc.Config.LocalPath)
	found := false
	for _, f := range files {
		if strings.HasSuffix(f.Name(), ".tar.gz.enc") {
			found = true
		}
	}
	if !found {
		t.Error("expected encrypted backup file in local path")
	}
}

func TestBackup_DumpFailure(t *testing.T) {
	svc, dumper, _, _, _, _, _ := newTestService(t)
	dumper.dumpErr = errors.New("database connection refused")

	result, _ := collectProgress(func(pf ProgressFunc) Result {
		return svc.Backup(pf)
	})

	if result.Success {
		t.Fatal("expected failure")
	}
	if !strings.Contains(result.Error, "database connection refused") {
		t.Errorf("expected dump error message, got %q", result.Error)
	}
}

func TestBackup_VerifyFailure(t *testing.T) {
	svc, dumper, _, _, _, _, _ := newTestService(t)
	dumper.verifyOK = false

	result, _ := collectProgress(func(pf ProgressFunc) Result {
		return svc.Backup(pf)
	})

	if result.Success {
		t.Fatal("expected failure on verification")
	}
	if !strings.Contains(result.Error, "verif") {
		t.Errorf("expected verification error, got %q", result.Error)
	}
}

func TestBackup_UploadFailure_LocalBackupStillCreated(t *testing.T) {
	svc, _, _, _, _, uploader, _ := newTestService(t)
	uploader.uploadErr = errors.New("network timeout")

	result, _ := collectProgress(func(pf ProgressFunc) Result {
		return svc.Backup(pf)
	})

	// Even if upload fails, local backup should still be created
	// and the result should indicate the local backup was made
	// The behavior is: local backup succeeds, upload failure is non-fatal
	if !result.Success {
		// Upload failure may be treated as non-fatal
		// Check that local file still exists
		files, _ := os.ReadDir(svc.Config.LocalPath)
		found := false
		for _, f := range files {
			if strings.HasSuffix(f.Name(), ".tar.gz.enc") {
				found = true
			}
		}
		if !found {
			t.Error("expected local backup file to exist even when upload fails")
		}
	}

	// Either way, local file should exist
	files, _ := os.ReadDir(svc.Config.LocalPath)
	found := false
	for _, f := range files {
		if strings.HasSuffix(f.Name(), ".tar.gz.enc") {
			found = true
		}
	}
	if !found {
		t.Error("expected local backup file to exist even when upload fails")
	}
}

func TestBackup_CatchUpUpload(t *testing.T) {
	svc, _, _, _, api, uploader, _ := newTestService(t)

	// Create some "local" backup files that are not in the cloud
	localFile1 := filepath.Join(svc.Config.LocalPath, "backup-20250101-120000.tar.gz.enc")
	localFile2 := filepath.Join(svc.Config.LocalPath, "backup-20250102-120000.tar.gz.enc")
	os.WriteFile(localFile1, []byte("encrypted-backup-1"), 0644)
	os.WriteFile(localFile2, []byte("encrypted-backup-2"), 0644)

	// Cloud already has localFile1 but not localFile2
	api.listBackupsResp = &ListBackupsResponse{
		Data: []BackupEntry{
			{Filename: "backup-20250101-120000.tar.gz.enc", DownloadURL: "https://example.com/1"},
		},
	}

	result, _ := collectProgress(func(pf ProgressFunc) Result {
		return svc.Backup(pf)
	})

	if !result.Success {
		t.Fatalf("expected success, got error: %s", result.Error)
	}

	// Should have uploaded localFile2 (catch-up) + the new backup
	uploadCalls := 0
	for _, call := range uploader.calls {
		if strings.HasPrefix(call, "upload:") {
			uploadCalls++
		}
	}
	// At least 2 uploads: catch-up for localFile2 + new backup
	if uploadCalls < 2 {
		t.Errorf("expected at least 2 uploads (catch-up + new), got %d", uploadCalls)
	}
}

func TestRestore_Success(t *testing.T) {
	svc, dumper, archiver, encryptor, api, uploader, _ := newTestService(t)

	// Set up directories for restore target
	restoreDir := t.TempDir()
	svc.Config.Directories = []string{restoreDir}

	api.getBackupResp = &BackupDetail{
		Filename:    "backup-20250101-120000.tar.gz.enc",
		DownloadURL: "https://storage.example.com/download/backup",
	}

	archiver.extractDirs = []string{restoreDir}

	var progress []string
	err := svc.Restore("backup-123", func(msg string) {
		progress = append(progress, msg)
	})

	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}

	// Verify API was called to get backup details
	foundGetBackup := false
	for _, call := range api.calls {
		if call == "get-backup:backup-123" {
			foundGetBackup = true
		}
	}
	if !foundGetBackup {
		t.Error("expected GetBackup to be called")
	}

	// Verify download was called
	if len(uploader.calls) < 1 {
		t.Fatal("expected download to be called")
	}

	// Verify decrypt was called
	if len(encryptor.calls) < 1 {
		t.Fatal("expected decrypt to be called")
	}

	// Verify extract was called
	if len(archiver.calls) < 1 {
		t.Fatal("expected extract to be called")
	}

	// Verify restore was called
	foundRestore := false
	for _, call := range dumper.calls {
		if strings.HasPrefix(call, "restore:") {
			foundRestore = true
		}
	}
	if !foundRestore {
		t.Error("expected database restore to be called")
	}

	// Verify progress messages
	if len(progress) == 0 {
		t.Error("expected progress messages")
	}
}

func TestRestore_WithLocalFile(t *testing.T) {
	svc, _, archiver, _, api, uploader, _ := newTestService(t)

	api.getBackupResp = &BackupDetail{
		Filename:    "backup-20250101-120000.tar.gz.enc",
		DownloadURL: "https://storage.example.com/download/backup",
	}

	archiver.extractDirs = []string{}

	// Create the file locally so download should be skipped
	localFile := filepath.Join(svc.Config.LocalPath, "backup-20250101-120000.tar.gz.enc")
	os.WriteFile(localFile, []byte("local-encrypted-data"), 0644)

	err := svc.Restore("backup-123", func(msg string) {})

	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}

	// Download should NOT have been called since file exists locally
	for _, call := range uploader.calls {
		if strings.HasPrefix(call, "download:") {
			t.Error("expected download to be skipped when file exists locally")
		}
	}
}

func TestRestore_APIError(t *testing.T) {
	svc, _, _, _, api, _, _ := newTestService(t)
	api.getBackupErr = errors.New("unauthorized")

	err := svc.Restore("backup-123", func(msg string) {})

	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "unauthorized") {
		t.Errorf("expected unauthorized error, got %q", err.Error())
	}
}

func TestRestore_DecryptError(t *testing.T) {
	svc, _, _, encryptor, api, _, _ := newTestService(t)

	api.getBackupResp = &BackupDetail{
		Filename:    "backup-20250101-120000.tar.gz.enc",
		DownloadURL: "https://storage.example.com/download/backup",
	}
	encryptor.decryptErr = errors.New("invalid encryption key")

	err := svc.Restore("backup-123", func(msg string) {})

	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "invalid encryption key") {
		t.Errorf("expected decryption error, got %q", err.Error())
	}
}

func TestBackup_NilProgressFunc(t *testing.T) {
	svc, _, _, _, _, _, _ := newTestService(t)

	// Should not panic with nil progress func
	result := svc.Backup(nil)

	if !result.Success {
		t.Fatalf("expected success, got error: %s", result.Error)
	}
}
