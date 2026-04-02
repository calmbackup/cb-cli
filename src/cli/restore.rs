use crate::cli::output::OutputMode;
use crate::core::types::Result;

/// Execute the restore command in CLI mode.
/// If backup_id is None and latest is false, launches interactive picker (if TTY).
pub async fn execute(
    config_path: Option<&str>,
    backup_id: Option<&str>,
    latest: bool,
    prune_local: bool,
    mode: OutputMode,
) -> Result<()> {
    todo!("Load config, resolve backup selection, run restore with CLI progress output")
}
