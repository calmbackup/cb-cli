use std::path::Path;
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
    todo!("Glob *.tar.gz.enc, filter by age + confirmed, delete, return count")
}
