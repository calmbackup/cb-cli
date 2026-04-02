use crate::cli::output::{self, OutputMode};
use crate::core::api::ApiClient;
use crate::core::backup::BackupService;
use crate::core::crypto;
use crate::core::dumper;
use crate::core::types::Result;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Execute the backup command in CLI mode.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    let config = output::load_config(config_path)?;

    if mode == OutputMode::Styled {
        output::print_header();
    }

    // Build the backup service
    let db_dumper = dumper::new_dumper(&config.database)?;
    let key = crypto::derive_key(&config.encryption_key);
    let api = ApiClient::new(&config.api_key, &config.api_url, VERSION);

    let service = BackupService {
        config: config.clone(),
        dumper: db_dumper,
        key,
        api,
    };

    // Build progress callback based on output mode
    let progress: Box<dyn Fn(&str, Option<&str>) + Send> = match mode {
        OutputMode::Styled => Box::new(|msg: &str, _detail: Option<&str>| {
            output::print_step(msg);
        }),
        _ => Box::new(|_msg: &str, _detail: Option<&str>| {}),
    };

    // Run the backup
    let result = match service.backup(progress).await {
        Ok(r) => r,
        Err(e) => {
            // Try to send failure notification (non-fatal)
            let notify_api = ApiClient::new(&config.api_key, &config.api_url, VERSION);
            let _ = notify_api.notify("backup-failed", &e.to_string()).await;
            return Err(e);
        }
    };

    match mode {
        OutputMode::Styled => {
            output::print_done(&format!("🔒 {}", result.filename));
            output::print_success(&format!(
                "Backup complete — {} in {:.1}s",
                crate::core::types::format_size(result.size),
                result.duration.as_secs_f64(),
            ));
            let now = chrono::Local::now();
            output::print_info(&format!(
                "{}",
                now.format("%b %-d, %Y %H:%M")
            ));
        }
        OutputMode::Json => {
            let json = serde_json::json!({
                "filename": result.filename,
                "size": result.size,
                "duration_secs": result.duration.as_secs_f64(),
                "checksum": result.checksum,
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        OutputMode::Quiet => {
            // Only errors are printed, nothing on success
        }
    }

    Ok(())
}
