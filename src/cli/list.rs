use crate::cli::output::OutputMode;
use crate::core::types::Result;

/// Execute the list command — display local and cloud backups.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    todo!("Load config, list local files + cloud backups, format output")
}
