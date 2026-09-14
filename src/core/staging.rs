//! Owner-only staging files; never expose a partial/unauthenticated destination.
use std::path::Path;
use tempfile::NamedTempFile;

use crate::core::types::Result;

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub fn file(destination: &Path) -> Result<NamedTempFile> {
    Ok(tempfile::Builder::new()
        .prefix(".calmbackup-")
        .tempfile_in(parent(destination))?)
}

pub fn publish(file: NamedTempFile, destination: &Path) -> Result<()> {
    file.as_file().sync_all()?;
    // Same-directory rename is atomic. If publication fails, NamedTempFile
    // removes the temporary file without changing an existing destination.
    file.persist(destination).map_err(|e| e.error)?;
    std::fs::File::open(parent(destination))?.sync_all()?;
    Ok(())
}
