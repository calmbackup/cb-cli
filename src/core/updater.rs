use std::io::Read;
use std::path::PathBuf;

use crate::core::types::{AppError, Result};

/// Check GitHub for the latest release version.
/// Returns (latest_tag, needs_update).
pub async fn check(current_version: &str) -> Result<(String, bool)> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/repos/calmbackup/cb-cli/releases/latest")
        .header("User-Agent", "calmbackup")
        .send()
        .await
        .map_err(|e| AppError::Api(format!("failed to check for updates: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Api(format!(
            "GitHub API returned status {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Api(format!("failed to parse release response: {e}")))?;

    let tag_name = body["tag_name"]
        .as_str()
        .ok_or_else(|| AppError::Api("missing tag_name in release response".into()))?
        .to_string();

    let latest = tag_name.strip_prefix('v').unwrap_or(&tag_name);
    let current = current_version
        .strip_prefix('v')
        .unwrap_or(current_version);
    let needs_update = latest != current;

    Ok((tag_name.clone(), needs_update))
}

/// Download and install the latest release, replacing the current binary.
/// Finds the correct tarball for the current OS/arch.
pub async fn update(latest_tag: &str) -> Result<()> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };

    let tag_version = latest_tag.strip_prefix('v').unwrap_or(latest_tag);
    let tarball_name = format!("calmbackup_{tag_version}_{os}_{arch}.tar.gz");
    let download_url = format!(
        "https://github.com/calmbackup/cb-cli/releases/download/{latest_tag}/{tarball_name}"
    );

    // Download the tarball
    let client = reqwest::Client::new();
    let resp = client
        .get(&download_url)
        .header("User-Agent", "calmbackup")
        .send()
        .await
        .map_err(|e| AppError::Api(format!("failed to download update: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Api(format!(
            "download failed with status {}",
            resp.status()
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Api(format!("failed to read download: {e}")))?;

    // Extract the binary from the tarball
    let temp_dir = std::env::temp_dir();
    let temp_binary = temp_dir.join("calmbackup_update");

    let decoder = flate2::read::GzDecoder::new(&bytes[..])
        .map_err(|e| AppError::Api(format!("failed to decompress tarball: {e}")))?;
    let mut archive = tar::Archive::new(decoder);

    let mut found = false;
    for entry in archive
        .entries()
        .map_err(|e| AppError::Api(format!("failed to read tarball: {e}")))?
    {
        let mut entry =
            entry.map_err(|e| AppError::Api(format!("failed to read tarball entry: {e}")))?;

        let path = entry
            .path()
            .map_err(|e| AppError::Api(format!("invalid path in tarball: {e}")))?
            .to_path_buf();

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if file_name == "calmbackup" {
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| AppError::Api(format!("failed to extract binary: {e}")))?;

            std::fs::write(&temp_binary, &contents)
                .map_err(|e| AppError::Api(format!("failed to write temp binary: {e}")))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&temp_binary, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| AppError::Api(format!("failed to set permissions: {e}")))?;
            }

            found = true;
            break;
        }
    }

    if !found {
        return Err(AppError::Api(
            "calmbackup binary not found in tarball".into(),
        ));
    }

    // Atomic replace: rename, falling back to copy + rename for cross-filesystem
    let current_exe =
        std::env::current_exe().map_err(|e| AppError::Api(format!("can't find current exe: {e}")))?;

    if std::fs::rename(&temp_binary, &current_exe).is_err() {
        // Cross-filesystem fallback: copy to a sibling temp file, then rename
        let staging = current_exe.with_extension("new");
        std::fs::copy(&temp_binary, &staging)
            .map_err(|e| AppError::Api(format!("failed to copy binary: {e}")))?;
        std::fs::rename(&staging, &current_exe)
            .map_err(|e| AppError::Api(format!("failed to replace binary: {e}")))?;
        let _ = std::fs::remove_file(&temp_binary);
    }

    Ok(())
}
