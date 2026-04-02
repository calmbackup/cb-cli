use std::path::Path;
use crate::core::api::ApiClient;
use crate::core::config::Config;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, ProgressFn, RestoreResult, Result};
use crate::core::{archive, crypto, upload};

/// Orchestrates the full restore pipeline.
pub struct RestoreService {
    pub config: Config,
    pub dumper: Box<dyn DatabaseDumper>,
    pub key: [u8; 32],
    pub api: ApiClient,
}

impl RestoreService {
    /// Execute the 7-step restore pipeline.
    ///
    /// 1. Fetch backup details (checksum, download URL)
    /// 2. Check local cache (verify checksum, skip download if valid)
    /// 3. Download from cloud if needed
    /// 4. Decrypt backup
    /// 5. Extract tar.gz archive
    /// 6. Restore database
    /// 7. Restore directories (walk extracted dirs back to original paths)
    pub async fn restore(
        &self,
        backup_id: &str,
        prune_local: bool,
        on_progress: ProgressFn,
    ) -> Result<RestoreResult> {
        let start = std::time::Instant::now();

        // Create temp directory
        let random_suffix: u32 = rand::random();
        let temp_dir = std::env::temp_dir().join(format!("calmbackup-restore-{:05}", random_suffix));
        std::fs::create_dir_all(&temp_dir)?;

        let result = self
            .restore_inner(backup_id, prune_local, &on_progress, &temp_dir)
            .await;

        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&temp_dir);

        let (bid, fname) = result?;
        Ok(RestoreResult {
            backup_id: bid,
            filename: fname,
            duration: start.elapsed(),
        })
    }

    async fn restore_inner(
        &self,
        backup_id: &str,
        prune_local: bool,
        on_progress: &(dyn Fn(&str, Option<&str>) + Send + Sync),
        temp_dir: &Path,
    ) -> Result<(String, String)> {
        // Step 1: Fetch backup details
        on_progress("Fetching backup details...", None);
        let backup = self.api.get_backup(backup_id).await?;
        let filename = &backup.filename;
        let cloud_checksum = backup.checksum.clone().unwrap_or_default();
        let download_url = backup.download_url.clone().ok_or_else(|| {
            AppError::Restore("No download URL available for this backup".to_string())
        })?;

        // Step 2: Check local cache
        on_progress("Checking local cache...", None);
        let local_dir = Path::new(&self.config.local_path);
        let local_path = local_dir.join(filename);
        let mut need_download = true;

        if local_path.exists() && !cloud_checksum.is_empty() {
            if let Ok(local_checksum) = crypto::checksum(&local_path) {
                if local_checksum == cloud_checksum {
                    need_download = false;
                }
            }
        }

        // Step 3: Download if needed
        if need_download {
            on_progress("Downloading backup...", None);
            std::fs::create_dir_all(local_dir)?;
            upload::download(&download_url, &local_path).await?;
        } else {
            on_progress("Downloading backup...", Some("cached locally"));
        }

        // Step 4: Decrypt
        on_progress("Decrypting backup...", None);
        let decrypted_path = temp_dir.join("archive.tar.gz");
        crypto::decrypt(&local_path, &decrypted_path, &self.key)?;

        // Step 5: Extract
        on_progress("Extracting archive...", None);
        let extract_dir = temp_dir.join("extracted");
        std::fs::create_dir_all(&extract_dir)?;
        archive::extract(&decrypted_path, &extract_dir)?;

        // Step 6: Restore database
        on_progress("Restoring database...", None);
        let dump_path = extract_dir.join(self.dumper.filename());
        if !dump_path.exists() {
            return Err(AppError::Restore(format!(
                "Dump file '{}' not found in archive",
                self.dumper.filename()
            )));
        }
        self.dumper.restore(&dump_path)?;

        // Step 7: Restore directories
        on_progress("Restoring directories...", None);
        self.restore_directories(&extract_dir)?;

        // Optionally prune the local cached file
        if prune_local {
            let _ = std::fs::remove_file(&local_path);
        }

        Ok((backup_id.to_string(), filename.clone()))
    }

    /// Walk the extract directory for directories matching config.directories basenames,
    /// and copy them back to their original paths.
    fn restore_directories(&self, extract_dir: &Path) -> Result<()> {
        for dir_config in &self.config.directories {
            let original_path = Path::new(dir_config);
            let basename = match original_path.file_name() {
                Some(name) => name,
                None => continue,
            };

            let extracted_dir = extract_dir.join(basename);
            if extracted_dir.is_dir() {
                // Recreate the original directory and copy contents
                std::fs::create_dir_all(original_path)?;
                copy_dir_recursive(&extracted_dir, original_path)?;
            }
        }
        Ok(())
    }
}

/// Recursively copy all files and directories from src to dst.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry_type.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
