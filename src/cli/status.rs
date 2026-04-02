use crate::cli::output::{self, OutputMode};
use crate::core::api::ApiClient;
use crate::core::types::{self, Result};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Execute the status command — show current backup state.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    let config = output::load_config(config_path)?;

    if mode == OutputMode::Styled {
        output::print_header();
    }

    // Scan local backups
    let local_path = std::path::Path::new(&config.local_path);
    let mut local_count: u64 = 0;
    let mut total_size: u64 = 0;
    let mut latest_filename: Option<String> = None;
    let mut latest_modified: Option<std::time::SystemTime> = None;

    if local_path.exists() {
        if let Ok(entries) = std::fs::read_dir(local_path) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if !fname.ends_with(".tar.gz.enc") {
                    continue;
                }
                if let Ok(meta) = std::fs::metadata(entry.path()) {
                    local_count += 1;
                    total_size += meta.len();
                    let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    if latest_modified.is_none() || modified > latest_modified.unwrap() {
                        latest_modified = Some(modified);
                        latest_filename = Some(fname);
                    }
                }
            }
        }
    }

    // Check API connectivity
    let api = ApiClient::new(&config.api_key, &config.api_url, VERSION);
    let api_healthy = api.health_check().await;

    // Get cloud backup count
    let cloud_count = if api_healthy {
        api.get_account().await.map(|a| a.backup_count).unwrap_or(0)
    } else {
        0
    };

    match mode {
        OutputMode::Json => {
            let json = serde_json::json!({
                "local_backup_count": local_count,
                "local_total_size": total_size,
                "latest_local_backup": latest_filename,
                "local_storage_path": config.local_path,
                "retention_days": config.local_retention_days,
                "api_connected": api_healthy,
                "cloud_backup_count": cloud_count,
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        OutputMode::Quiet => {}
        OutputMode::Styled => {
            output::print_section("Local");
            output::print_label("Backups", &local_count.to_string());
            output::print_label("Total size", &types::format_size(total_size));
            if let Some(ref fname) = latest_filename {
                output::print_label("Latest", &format!("🔒 {}", fname));
            } else {
                output::print_label("Latest", "—");
            }
            output::print_label("Storage path", &config.local_path);
            output::print_label("Retention", &format!("{} days", config.local_retention_days));

            output::print_section("Cloud");
            output::print_label(
                "API",
                if api_healthy { "connected" } else { "unreachable" },
            );
            output::print_label("Cloud backups", &cloud_count.to_string());
            println!();
        }
    }

    Ok(())
}
