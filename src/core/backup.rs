use std::path::Path;
use crate::core::api::ApiClient;
use crate::core::config::Config;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{BackupResult, ProgressFn, Result};

/// Orchestrates the full backup pipeline.
pub struct BackupService {
    pub config: Config,
    pub dumper: Box<dyn DatabaseDumper>,
    pub key: [u8; 32],
    pub api: ApiClient,
}

impl BackupService {
    /// Execute the 10-step backup pipeline.
    ///
    /// 1. Create temp directory
    /// 2. Dump database
    /// 3. Verify dump
    /// 4. Create tar.gz archive (dump + directories)
    /// 5. Encrypt archive → backup-YYYYMMDD-HHMMSS.tar.gz.enc
    /// 6. Catch-up upload (sync unuploaded local backups)
    /// 7. Save encrypted file to local_path
    /// 8. Compute SHA-256 checksum
    /// 9. Upload to cloud (request URL, PUT, confirm)
    /// 10. Prune old local backups
    pub async fn backup(&self, on_progress: ProgressFn) -> Result<BackupResult> {
        todo!("Implement 10-step backup pipeline")
    }
}
