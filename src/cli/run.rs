use crate::cli::output::OutputMode;
use crate::core::types::Result;

/// Execute the backup command in CLI mode.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    todo!("Load config, build BackupService, run backup with CLI progress output")
}
