use std::fs::OpenOptions;
use std::io::Read;
use std::time::Duration;

use fs2::FileExt;
use semver::Version;
use sha2::{Digest, Sha256};

use crate::core::types::{AppError, Result};

/// Check GitHub for the latest release version.
/// Returns (latest_tag, needs_update).
pub async fn check(current_version: &str) -> Result<(String, bool)> {
    let client = http_client(Duration::from_secs(5))?;
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

    let latest = parse_version(&tag_name)?;
    let current = parse_version(current_version)?;
    let needs_update = latest > current;

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
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };

    let tag_version = latest_tag.strip_prefix('v').unwrap_or(latest_tag);
    let tarball_name = format!("calmbackup_{tag_version}_{os}_{arch}.tar.gz");
    let download_url = format!(
        "https://github.com/calmbackup/cb-cli/releases/download/{latest_tag}/{tarball_name}"
    );
    let checksum_url =
        format!("https://github.com/calmbackup/cb-cli/releases/download/{latest_tag}/SHA256SUMS");

    // Only one process may replace the executable. This also protects hosts
    // with several independently scheduled CalmBackup sources.
    let current_exe = std::env::current_exe()
        .map_err(|e| AppError::Api(format!("can't find current exe: {e}")))?;
    let lock_path = current_exe.with_extension("update.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| AppError::Api(format!("failed to open update lock: {e}")))?;
    lock.try_lock_exclusive()
        .map_err(|e| AppError::Api(format!("another CalmBackup process is updating: {e}")))?;

    // Download the tarball
    let client = http_client(Duration::from_secs(60))?;
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

    // Release checksums are published alongside every binary. Never execute
    // or install a downloaded archive that cannot be authenticated this way.
    let checksum_resp = client
        .get(&checksum_url)
        .header("User-Agent", "calmbackup")
        .send()
        .await
        .map_err(|e| AppError::Api(format!("failed to download checksums: {e}")))?;

    if !checksum_resp.status().is_success() {
        return Err(AppError::Api(format!(
            "checksum download failed with status {}",
            checksum_resp.status()
        )));
    }

    let checksums = checksum_resp
        .text()
        .await
        .map_err(|e| AppError::Api(format!("failed to read checksums: {e}")))?;
    verify_checksum(&tarball_name, &bytes, &checksums)?;

    // Extract the binary from the tarball
    let temp_binary =
        std::env::temp_dir().join(format!("calmbackup_update_{}", std::process::id()));

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

    // Atomic replace: rename, falling back to copy + rename for cross-filesystem.
    // Keep the previous executable beside it for manual rollback.
    let previous = current_exe.with_extension("previous");
    std::fs::copy(&current_exe, &previous)
        .map_err(|e| AppError::Api(format!("failed to preserve previous binary: {e}")))?;
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

fn parse_version(value: &str) -> Result<Version> {
    Version::parse(value.strip_prefix('v').unwrap_or(value))
        .map_err(|e| AppError::Api(format!("invalid release version {value}: {e}")))
}

fn http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| AppError::Api(format!("failed to create update client: {e}")))
}

fn verify_checksum(filename: &str, bytes: &[u8], checksums: &str) -> Result<()> {
    let expected = checksums
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?, fields.next()?.trim_start_matches('*')))
        })
        .find_map(|(checksum, name)| (name == filename).then_some(checksum))
        .ok_or_else(|| AppError::Api(format!("no checksum published for {filename}")))?;
    let actual = hex::encode(Sha256::digest(bytes));

    if !actual.eq_ignore_ascii_case(expected) {
        return Err(AppError::Api(format!("checksum mismatch for {filename}")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_does_not_downgrade() {
        assert!(parse_version("v2.1.0").unwrap() > parse_version("2.0.9").unwrap());
        assert!(parse_version("2.0.8").unwrap() < parse_version("2.0.9").unwrap());
    }

    #[test]
    fn verifies_named_archive_checksum() {
        let bytes = b"release archive";
        let digest = hex::encode(Sha256::digest(bytes));
        let manifest = format!("{digest}  calmbackup_2.0.10_linux_amd64.tar.gz\n");

        verify_checksum("calmbackup_2.0.10_linux_amd64.tar.gz", bytes, &manifest).unwrap();
        assert!(verify_checksum("other.tar.gz", bytes, &manifest).is_err());
        assert!(
            verify_checksum(
                "calmbackup_2.0.10_linux_amd64.tar.gz",
                b"tampered",
                &manifest
            )
            .is_err()
        );
    }
}
