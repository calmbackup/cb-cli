use crate::core::types::{AccountInfo, AppError, BackupEntry, Result, UploadUrlResponse};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};

/// Wrapper for paginated API responses: {"data": [...], "meta": {...}}
#[derive(serde::Deserialize)]
struct PaginatedResponse<T> {
    data: Vec<T>,
}

/// HTTP client for the CalmBackup API.
pub struct ApiClient {
    api_key: String,
    base_url: String,
    version: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(api_key: &str, base_url: &str, version: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .expect("invalid api key for header"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "X-Backup-Version",
            HeaderValue::from_str(version).expect("invalid version for header"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build reqwest client");

        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            version: version.to_string(),
            client,
        }
    }

    /// Check an HTTP response status and map error codes to AppError variants.
    async fn check_response(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();

        if status.is_success() {
            return Ok(response);
        }

        match status.as_u16() {
            401 => Err(AppError::Authentication),
            402 => Err(AppError::Billing),
            409 => Err(AppError::BackupDeleted),
            413 => Err(AppError::SizeLimit),
            422 => {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown validation error".to_string());
                Err(AppError::Validation(body))
            }
            429 => Err(AppError::RateLimit),
            code if code >= 500 => Err(AppError::Server(format!("status {}", code))),
            _ => Err(AppError::Api(format!("unexpected status {}", status))),
        }
    }

    /// Request a presigned upload URL for a new backup.
    pub async fn request_upload_url(
        &self,
        filename: &str,
        size: u64,
        checksum: &str,
        db_driver: &str,
    ) -> Result<UploadUrlResponse> {
        let url = format!("{}/upload-url", self.base_url);
        let body = serde_json::json!({
            "filename": filename,
            "size": size,
            "checksum": checksum,
            "db_driver": db_driver,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        let response = self.check_response(response).await?;
        response
            .json::<UploadUrlResponse>()
            .await
            .map_err(|e| AppError::Api(e.to_string()))
    }

    /// Confirm a backup upload was successful.
    pub async fn confirm_backup(
        &self,
        backup_id: &str,
        size: u64,
        checksum: &str,
    ) -> Result<()> {
        let url = format!("{}/backups/{}/confirm", self.base_url, backup_id);
        let body = serde_json::json!({
            "size": size,
            "checksum": checksum,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        self.check_response(response).await?;
        Ok(())
    }

    /// List backups with pagination.
    pub async fn list_backups(&self, page: u32, per_page: u32) -> Result<Vec<BackupEntry>> {
        let url = format!(
            "{}/backups?page={}&per_page={}",
            self.base_url, page, per_page
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        let response = self.check_response(response).await?;
        let paginated: PaginatedResponse<BackupEntry> = response
            .json()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;
        Ok(paginated.data)
    }

    /// Get a single backup's details including download URL.
    pub async fn get_backup(&self, backup_id: &str) -> Result<BackupEntry> {
        let url = format!("{}/backups/{}", self.base_url, backup_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        let response = self.check_response(response).await?;
        response
            .json::<BackupEntry>()
            .await
            .map_err(|e| AppError::Api(e.to_string()))
    }

    /// Delete a cloud backup.
    pub async fn delete_backup(&self, backup_id: &str) -> Result<()> {
        let url = format!("{}/backups/{}", self.base_url, backup_id);

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        self.check_response(response).await?;
        Ok(())
    }

    /// Send a notification event (e.g., backup-failed).
    pub async fn notify(&self, event: &str, reason: &str) -> Result<()> {
        let url = format!("{}/notify/{}", self.base_url, event);
        let body = serde_json::json!({
            "reason": reason,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        self.check_response(response).await?;
        Ok(())
    }

    /// Get account info (count, storage, last backup).
    pub async fn get_account(&self) -> Result<AccountInfo> {
        let url = format!("{}/account", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Api(e.to_string()))?;

        let response = self.check_response(response).await?;
        response
            .json::<AccountInfo>()
            .await
            .map_err(|e| AppError::Api(e.to_string()))
    }

    /// Check API connectivity (returns true if reachable).
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/account", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}
