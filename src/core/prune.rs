use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::core::types::Result;

/// Prune old local backups that are confirmed to exist in the cloud.
///
/// Only deletes files that:
/// 1. Match *.tar.gz.enc pattern
/// 2. Are older than `retention_days`
/// 3. Have their filename in the `confirmed_filenames` set (exist in cloud)
///
/// Returns the count of deleted files.
pub fn prune(
    backup_dir: &Path,
    retention_days: u32,
    confirmed_filenames: &[String],
) -> Result<u32> {
    let confirmed: HashSet<&str> = confirmed_filenames.iter().map(|s| s.as_str()).collect();
    let cutoff = SystemTime::now() - Duration::from_secs(u64::from(retention_days) * 24 * 60 * 60);
    let mut deleted = 0u32;

    // This propagates AppError::Io if the directory can't be read
    let entries = std::fs::read_dir(backup_dir)?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // skip individual read errors
        };

        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Filter to *.tar.gz.enc files only
        if !filename_str.ends_with(".tar.gz.enc") {
            continue;
        }

        // Check modification time — skip on error
        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Must be older than retention period AND confirmed in the cloud
        if modified < cutoff && confirmed.contains(filename_str.as_ref()) {
            if let Err(_) = std::fs::remove_file(entry.path()) {
                // Skip files that fail to delete
                continue;
            }
            deleted += 1;
        }
    }

    Ok(deleted)
}
