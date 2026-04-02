use crate::core::types::{AccountInfo, BackupEntry, Result, UploadUrlResponse};

/// HTTP client for the CalmBackup API.
pub struct ApiClient {
    api_key: String,
    base_url: String,
    version: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(api_key: &str, base_url: &str, version: &str) -> Self {
        todo!("Create reqwest client with default headers")
    }

    /// Request a presigned upload URL for a new backup.
    pub async fn request_upload_url(
        &self,
        filename: &str,
        size: u64,
        checksum: &str,
        db_driver: &str,
    ) -> Result<UploadUrlResponse> {
        todo!("POST /upload-url")
    }

    /// Confirm a backup upload was successful.
    pub async fn confirm_backup(
        &self,
        backup_id: &str,
        size: u64,
        checksum: &str,
    ) -> Result<()> {
        todo!("POST /backups/{id}/confirm")
    }

    /// List backups with pagination.
    pub async fn list_backups(&self, page: u32, per_page: u32) -> Result<Vec<BackupEntry>> {
        todo!("GET /backups?page=X&per_page=Y")
    }

    /// Get a single backup's details including download URL.
    pub async fn get_backup(&self, backup_id: &str) -> Result<BackupEntry> {
        todo!("GET /backups/{id}")
    }

    /// Delete a cloud backup.
    pub async fn delete_backup(&self, backup_id: &str) -> Result<()> {
        todo!("DELETE /backups/{id}")
    }

    /// Send a notification event (e.g., backup-failed).
    pub async fn notify(&self, event: &str, reason: &str) -> Result<()> {
        todo!("POST /notify/{event}")
    }

    /// Get account info (count, storage, last backup).
    pub async fn get_account(&self) -> Result<AccountInfo> {
        todo!("GET /account")
    }

    /// Check API connectivity (returns true if reachable).
    pub async fn health_check(&self) -> bool {
        todo!("GET /account, return true if 2xx")
    }
}
