use std::path::Path;
use crate::core::types::Result;

/// Upload an encrypted backup file to a presigned URL via HTTP PUT.
pub async fn upload(file_path: &Path, presigned_url: &str) -> Result<()> {
    todo!("Open file, PUT to presigned URL with Content-Type: application/octet-stream")
}

/// Download a backup file from a presigned URL via HTTP GET.
/// Removes partial file on failure.
pub async fn download(url: &str, output_path: &Path) -> Result<()> {
    todo!("GET from URL, stream to file, remove partial on error")
}
