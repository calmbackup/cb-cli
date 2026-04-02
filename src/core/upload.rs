use std::path::Path;

use crate::core::types::{AppError, Result};
use tokio::io::AsyncWriteExt;

/// Upload an encrypted backup file to a presigned URL via HTTP PUT.
pub async fn upload(file_path: &Path, presigned_url: &str) -> Result<()> {
    let file_data = tokio::fs::read(file_path)
        .await
        .map_err(|e| AppError::Upload(format!("failed to read file: {}", e)))?;

    let size = file_data.len();

    let client = reqwest::Client::new();
    let response = client
        .put(presigned_url)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", size)
        .body(file_data)
        .send()
        .await
        .map_err(|e| AppError::Upload(format!("upload request failed: {}", e)))?;

    if response.status().as_u16() >= 400 {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(AppError::Upload(format!(
            "upload failed with status {}: {}",
            status, body
        )));
    }

    Ok(())
}

/// Download a backup file from a presigned URL via HTTP GET.
/// Removes partial file on failure.
pub async fn download(url: &str, output_path: &Path) -> Result<()> {
    let result = download_inner(url, output_path).await;

    if result.is_err() {
        // Remove partial file on failure, ignoring removal errors.
        let _ = tokio::fs::remove_file(output_path).await;
    }

    result
}

async fn download_inner(url: &str, output_path: &Path) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Download(format!("download request failed: {}", e)))?;

    if response.status().as_u16() >= 400 {
        return Err(AppError::Download(format!(
            "download failed with status {}",
            response.status()
        )));
    }

    let mut file = tokio::fs::File::create(output_path)
        .await
        .map_err(|e| AppError::Download(format!("failed to create output file: {}", e)))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Download(format!("failed to read response body: {}", e)))?;

    file.write_all(&bytes)
        .await
        .map_err(|e| AppError::Download(format!("failed to write to file: {}", e)))?;

    file.flush()
        .await
        .map_err(|e| AppError::Download(format!("failed to flush file: {}", e)))?;

    Ok(())
}
