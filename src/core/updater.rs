use crate::core::types::Result;

/// Check GitHub for the latest release version.
/// Returns (latest_tag, needs_update).
pub async fn check(current_version: &str) -> Result<(String, bool)> {
    todo!("GET https://api.github.com/repos/calmbackup/cb-cli/releases/latest, compare versions")
}

/// Download and install the latest release, replacing the current binary.
/// Finds the correct tarball for the current OS/arch.
pub async fn update(latest_tag: &str) -> Result<()> {
    todo!("Download tarball for OS/arch, extract binary, atomic replace")
}
