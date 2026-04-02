use std::path::{Path, PathBuf};
use crate::core::types::{AppError, Result};

/// Create a tar.gz archive containing the database dump and optional directories.
/// The dump file is placed at the archive root (renamed to its basename).
/// Each directory is added as a top-level folder (by basename).
pub fn create(dump_path: &Path, directories: &[String], output_path: &Path) -> Result<()> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs::File;

    let file = File::create(output_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    // Add the dump file at the archive root using its basename
    let dump_name = dump_path
        .file_name()
        .ok_or_else(|| AppError::Archive("dump path has no filename".to_string()))?;
    builder
        .append_path_with_name(dump_path, dump_name)
        .map_err(|e| AppError::Archive(format!("failed to add dump file: {}", e)))?;

    // Add each directory recursively as a top-level folder by basename
    for dir in directories {
        let dir_path = Path::new(dir);
        let dir_name = dir_path
            .file_name()
            .ok_or_else(|| AppError::Archive(format!("directory has no basename: {}", dir)))?;
        builder
            .append_dir_all(dir_name, dir_path)
            .map_err(|e| AppError::Archive(format!("failed to add directory {}: {}", dir, e)))?;
    }

    // Finish writing the archive
    let encoder = builder
        .into_inner()
        .map_err(|e| AppError::Archive(format!("failed to finish archive: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| AppError::Archive(format!("failed to finish gzip: {}", e)))?;

    Ok(())
}

/// Extract a tar.gz archive to the given output directory.
/// Returns the list of extracted file paths.
/// Sanitizes paths to prevent directory traversal attacks.
pub fn extract(archive_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    use flate2::read::GzDecoder;
    use std::fs::File;

    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);

    let mut extracted = Vec::new();

    for entry in archive
        .entries()
        .map_err(|e| AppError::Archive(format!("failed to read archive entries: {}", e)))?
    {
        let mut entry =
            entry.map_err(|e| AppError::Archive(format!("failed to read entry: {}", e)))?;

        let path = entry
            .path()
            .map_err(|e| AppError::Archive(format!("failed to read entry path: {}", e)))?
            .to_path_buf();

        // Reject any path containing ".." components
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                return Err(AppError::Archive(format!(
                    "path traversal detected in archive entry: {}",
                    path.display()
                )));
            }
        }

        let full_path = output_dir.join(&path);
        entry
            .unpack(&full_path)
            .map_err(|e| AppError::Archive(format!("failed to unpack {}: {}", path.display(), e)))?;

        extracted.push(full_path);
    }

    Ok(extracted)
}
