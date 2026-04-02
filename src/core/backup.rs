use std::path::Path;
use crate::core::api::ApiClient;
use crate::core::config::Config;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, BackupResult, ProgressFn, Result};
use crate::core::{archive, crypto, prune, upload};

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
        let start = std::time::Instant::now();

        // Step 1: Create temp directory
        let random_suffix: u32 = rand::random();
        let temp_dir = std::env::temp_dir().join(format!("calmbackup-{:05}", random_suffix));
        std::fs::create_dir_all(&temp_dir)?;

        // Ensure cleanup on all exit paths
        let result = self.backup_inner(&temp_dir, &on_progress).await;

        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&temp_dir);

        let backup_result = result?;
        Ok(BackupResult {
            filename: backup_result.0,
            size: backup_result.1,
            duration: start.elapsed(),
            checksum: backup_result.2,
        })
    }

    async fn backup_inner(
        &self,
        temp_dir: &Path,
        on_progress: &dyn Fn(&str, Option<&str>),
    ) -> Result<(String, u64, String)> {
        // Step 2: Dump database
        on_progress("Dumping database...", None);
        let dump_path = temp_dir.join(self.dumper.filename());
        self.dumper.dump(&dump_path)?;

        // Step 3: Verify dump
        on_progress("Verifying dump...", None);
        let valid = self.dumper.verify(&dump_path)?;
        if !valid {
            return Err(AppError::DumpVerify("Dump verification failed".to_string()));
        }

        // Step 4: Create archive
        on_progress("Creating archive...", None);
        let archive_path = temp_dir.join("archive.tar.gz");
        archive::create(&dump_path, &self.config.directories, &archive_path)?;

        // Step 5: Encrypt
        on_progress("Encrypting backup...", None);
        let now = chrono::Local::now();
        let filename = format!("backup-{}.tar.gz.enc", now.format("%Y%m%d-%H%M%S"));
        let encrypted_path = temp_dir.join(&filename);
        crypto::encrypt(&archive_path, &encrypted_path, &self.key)?;

        // Step 6: Catch-up upload (best-effort)
        on_progress("Syncing backups...", None);
        let local_dir = Path::new(&self.config.local_path);
        if let Ok(()) = self.catchup_upload(local_dir).await {}

        // Step 7: Save locally
        on_progress("Saving locally...", None);
        std::fs::create_dir_all(local_dir)?;
        let local_path = local_dir.join(&filename);
        std::fs::copy(&encrypted_path, &local_path)?;

        // Step 8: Compute checksum
        on_progress("Computing checksum...", None);
        let checksum = crypto::checksum(&local_path)?;

        // Step 9: Upload to cloud
        on_progress("Uploading to cloud...", None);
        let size = std::fs::metadata(&local_path)?.len();
        match self.upload_to_cloud(&local_path, &filename, size, &checksum).await {
            Ok(()) => {}
            Err(AppError::BackupDeleted) => {
                // Non-fatal: backup was deleted server-side during upload
            }
            Err(e) => return Err(e),
        }

        // Step 10: Prune old local backups
        on_progress("Pruning old backups...", None);
        let cloud_backups = self.api.list_backups(1, 100).await.unwrap_or_default();
        let confirmed: Vec<String> = cloud_backups.iter().map(|b| b.filename.clone()).collect();
        let _ = prune::prune(local_dir, self.config.local_retention_days, &confirmed);

        Ok((filename, size, checksum))
    }

    /// Upload a single file to the cloud via the API flow.
    async fn upload_to_cloud(
        &self,
        local_path: &Path,
        filename: &str,
        size: u64,
        checksum: &str,
    ) -> Result<()> {
        let upload_resp = self
            .api
            .request_upload_url(filename, size, checksum, &self.config.database.driver)
            .await?;

        upload::upload(local_path, &upload_resp.upload_url).await?;

        self.api
            .confirm_backup(&upload_resp.backup_id, size, checksum)
            .await?;

        Ok(())
    }

    /// Catch-up upload: sync local .tar.gz.enc files not yet in the cloud.
    /// Best-effort — errors are silently ignored.
    async fn catchup_upload(&self, local_dir: &Path) -> Result<()> {
        if !local_dir.exists() {
            return Ok(());
        }

        let cloud_backups = self.api.list_backups(1, 100).await?;
        let cloud_filenames: std::collections::HashSet<String> =
            cloud_backups.iter().map(|b| b.filename.clone()).collect();

        let entries = std::fs::read_dir(local_dir)?;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            if !fname_str.ends_with(".tar.gz.enc") {
                continue;
            }

            if cloud_filenames.contains(fname_str.as_ref()) {
                continue;
            }

            let path = entry.path();
            let size = match std::fs::metadata(&path) {
                Ok(m) => m.len(),
                Err(_) => continue,
            };
            let checksum = match crypto::checksum(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Best-effort upload, ignore errors
            let _ = self
                .upload_to_cloud(&path, &fname_str, size, &checksum)
                .await;
        }

        Ok(())
    }
}
