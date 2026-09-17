use crate::cli::output::{self, OutputMode};
use crate::core::{api::ApiClient, crypto, types::Result, upload_existing};
use std::path::Path;

pub async fn execute(
    config_path: Option<&str>,
    path: &Path,
    checksum: &str,
    mode: OutputMode,
) -> Result<()> {
    let config = output::load_config(config_path)?;
    let api = ApiClient::new(&config.api_key, &config.api_url, env!("CARGO_PKG_VERSION"));
    let receipt = upload_existing::execute(
        &api,
        path,
        checksum,
        &crypto::derive_key(&config.encryption_key),
        &config.database.driver,
    )
    .await?;
    match mode {
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(&receipt).unwrap()),
        OutputMode::Styled => output::print_success(&format!(
            "Encrypted archive confirmed: {}. Restore verification is still required.",
            receipt.id
        )),
        OutputMode::Quiet => {}
    }
    Ok(())
}
