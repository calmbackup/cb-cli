use std::path::{Path, PathBuf};
use crate::core::types::Result;

/// Create a tar.gz archive containing the database dump and optional directories.
/// The dump file is placed at the archive root (renamed to its basename).
/// Each directory is added as a top-level folder (by basename).
pub fn create(dump_path: &Path, directories: &[String], output_path: &Path) -> Result<()> {
    todo!("Create tar.gz with dump + directories, preserving permissions and timestamps")
}

/// Extract a tar.gz archive to the given output directory.
/// Returns the list of extracted file paths.
/// Sanitizes paths to prevent directory traversal attacks.
pub fn extract(archive_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    todo!("Extract tar.gz, sanitize paths (reject ..), return extracted paths")
}
