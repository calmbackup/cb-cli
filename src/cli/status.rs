use crate::cli::output::OutputMode;
use crate::core::types::Result;

/// Execute the status command — show current backup state.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    todo!("Load config, show local count/size, latest file, retention, API connectivity")
}
