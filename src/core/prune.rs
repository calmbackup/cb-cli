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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Create a unique temp directory for test isolation.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cb_prune_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn deletes_old_confirmed_files() {
        let dir = temp_dir("delete_old");
        let filenames = vec![
            "backup_20260101.tar.gz.enc".to_string(),
            "backup_20260102.tar.gz.enc".to_string(),
        ];
        for f in &filenames {
            fs::write(dir.join(f), "encrypted data").unwrap();
        }

        // retention_days=0 means cutoff is now, so any file created before now qualifies
        let deleted = prune(&dir, 0, &filenames).unwrap();
        assert_eq!(deleted, 2);
        assert!(!dir.join(&filenames[0]).exists());
        assert!(!dir.join(&filenames[1]).exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_recent_files() {
        let dir = temp_dir("keep_recent");
        let filenames = vec!["recent.tar.gz.enc".to_string()];
        fs::write(dir.join(&filenames[0]), "data").unwrap();

        // retention_days=9999 means cutoff is far in the past; files just created are recent
        let deleted = prune(&dir, 9999, &filenames).unwrap();
        assert_eq!(deleted, 0);
        assert!(dir.join(&filenames[0]).exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_unconfirmed_files() {
        let dir = temp_dir("keep_unconfirmed");
        let filename = "unconfirmed.tar.gz.enc";
        fs::write(dir.join(filename), "data").unwrap();

        // Pass empty confirmed list — file should not be deleted even with retention_days=0
        let deleted = prune(&dir, 0, &[]).unwrap();
        assert_eq!(deleted, 0);
        assert!(dir.join(filename).exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_non_matching_extensions() {
        let dir = temp_dir("non_matching");
        let files = vec!["notes.txt", "backup.sql", "data.tar.gz", "readme.md"];
        for f in &files {
            fs::write(dir.join(f), "content").unwrap();
        }

        let confirmed: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        let deleted = prune(&dir, 0, &confirmed).unwrap();
        assert_eq!(deleted, 0);

        // All files should still exist
        for f in &files {
            assert!(dir.join(f).exists());
        }

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_directory() {
        let dir = temp_dir("empty");
        let deleted = prune(&dir, 0, &[]).unwrap();
        assert_eq!(deleted, 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nonexistent_directory() {
        let dir = std::env::temp_dir().join("cb_prune_test_nonexistent_xyz");
        let _ = fs::remove_dir_all(&dir); // ensure it doesn't exist
        let result = prune(&dir, 7, &[]);
        assert!(result.is_err());
    }
}
