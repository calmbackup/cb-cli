use std::io::IsTerminal;

use crate::cli::output::{self, OutputMode};
use crate::core::api::ApiClient;
use crate::core::crypto;
use crate::core::dumper;
use crate::core::restore::RestoreService;
use crate::core::types::{AppError, BackupEntry, Result};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Execute the restore command in CLI mode.
/// If backup_id is None and latest is false, launches interactive picker (if TTY).
pub async fn execute(
    config_path: Option<&str>,
    backup_id: Option<&str>,
    latest: bool,
    prune_local: bool,
    mode: OutputMode,
) -> Result<()> {
    let config = output::load_config(config_path)?;

    if mode == OutputMode::Styled {
        output::print_header();
    }

    let api = ApiClient::new(&config.api_key, &config.api_url, VERSION);

    // Resolve which backup to restore
    let selected: BackupEntry = if let Some(id) = backup_id {
        // Explicit backup ID provided
        api.get_backup(id).await?
    } else if latest {
        // Use the latest backup
        let backups = api.list_backups(1, 1).await?;
        backups.into_iter().next().ok_or_else(|| {
            AppError::Restore("No backups found.".into())
        })?
    } else {
        // Interactive picker or error
        if !std::io::stdin().is_terminal() {
            return Err(AppError::Restore(
                "No backup specified. Use --latest or provide a backup ID.".into(),
            ));
        }

        let backups = api.list_backups(1, 20).await?;
        if backups.is_empty() {
            return Err(AppError::Restore("No backups found.".into()));
        }

        // Print numbered list
        output::print_section("Select a backup to restore");
        for (i, b) in backups.iter().enumerate() {
            let time_str = crate::core::types::format_time(&b.created_at);
            let size_str = crate::core::types::format_size(b.size);
            output::print_info(&format!(
                "  {}. 🔒 {}  {}  {}",
                i + 1,
                b.filename,
                size_str,
                time_str,
            ));
        }
        println!();

        let selection = output::prompt("Enter number: ")?;
        let idx: usize = selection
            .parse::<usize>()
            .map_err(|_| AppError::Restore("Invalid selection.".into()))?;

        if idx == 0 || idx > backups.len() {
            return Err(AppError::Restore("Selection out of range.".into()));
        }

        backups[idx - 1].clone()
    };

    // Confirmation prompt if TTY
    if std::io::stdin().is_terminal() && mode == OutputMode::Styled {
        output::print_info(&format!("🔒 {}", selected.filename));
        let confirm = output::prompt("Restore this backup? (yes/no): ")?;
        if confirm != "yes" && confirm != "y" {
            output::print_info("Restore cancelled.");
            return Ok(());
        }
    }

    // Build restore service
    let db_dumper = dumper::new_dumper(&config.database)?;
    let key = crypto::derive_key(&config.encryption_key);
    let restore_api = ApiClient::new(&config.api_key, &config.api_url, VERSION);

    let service = RestoreService {
        config: config.clone(),
        dumper: db_dumper,
        key,
        api: restore_api,
    };

    // Build progress callback
    let progress: Box<dyn Fn(&str, Option<&str>) + Send> = match mode {
        OutputMode::Styled => Box::new(|msg: &str, _detail: Option<&str>| {
            output::print_step(msg);
        }),
        _ => Box::new(|_msg: &str, _detail: Option<&str>| {}),
    };

    // Run restore
    let result = match service.restore(&selected.id, prune_local, progress).await {
        Ok(r) => r,
        Err(e) => {
            let notify_api = ApiClient::new(&config.api_key, &config.api_url, VERSION);
            let _ = notify_api.notify("restore-failed", &e.to_string()).await;
            return Err(e);
        }
    };

    match mode {
        OutputMode::Styled => {
            output::print_done(&format!("🔒 {}", result.filename));
            output::print_success(&format!(
                "Restore complete in {:.1}s",
                result.duration.as_secs_f64(),
            ));
        }
        OutputMode::Json => {
            let json = serde_json::json!({
                "backup_id": result.backup_id,
                "filename": result.filename,
                "duration_secs": result.duration.as_secs_f64(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        OutputMode::Quiet => {}
    }

    Ok(())
}
