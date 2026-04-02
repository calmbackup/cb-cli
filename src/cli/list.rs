use crate::cli::output::{self, OutputMode};
use crate::core::api::ApiClient;
use crate::core::types::{self, LocalBackupInfo, Result};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scan the local backup directory for .tar.gz.enc files.
fn scan_local_backups(local_path: &str) -> Vec<LocalBackupInfo> {
    let dir = std::path::Path::new(local_path);
    if !dir.exists() {
        return Vec::new();
    }

    let mut backups: Vec<LocalBackupInfo> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.ends_with(".tar.gz.enc") {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                backups.push(LocalBackupInfo {
                    path: path.clone(),
                    filename,
                    size: meta.len(),
                    modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                });
            }
        }
    }

    // Sort by modified time, newest first
    backups.sort_by(|a, b| b.modified.cmp(&a.modified));
    backups
}

/// Format a SystemTime into display format.
fn format_system_time(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%b %-d, %Y %H:%M").to_string()
}

/// Execute the list command — display local and cloud backups.
pub async fn execute(config_path: Option<&str>, mode: OutputMode) -> Result<()> {
    let config = output::load_config(config_path)?;

    if mode == OutputMode::Styled {
        output::print_header();
    }

    // Scan local backups
    let local_backups = scan_local_backups(&config.local_path);

    // Fetch cloud backups
    let api = ApiClient::new(&config.api_key, &config.api_url, VERSION);
    let cloud_backups = api.list_backups(1, 50).await.unwrap_or_default();

    match mode {
        OutputMode::Json => {
            let local_json: Vec<serde_json::Value> = local_backups
                .iter()
                .map(|b| {
                    serde_json::json!({
                        "filename": b.filename,
                        "size": b.size,
                        "modified": format_system_time(b.modified),
                    })
                })
                .collect();

            let json = serde_json::json!({
                "local_backups": local_json,
                "cloud_backups": cloud_backups,
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        OutputMode::Quiet => {}
        OutputMode::Styled => {
            // Local backups section
            output::print_section("Local Backups");
            if local_backups.is_empty() {
                output::print_info("No local backups found.");
            } else {
                let total_size: u64 = local_backups.iter().map(|b| b.size).sum();
                output::print_info(&format!(
                    "{} backup{}, {} total",
                    local_backups.len(),
                    if local_backups.len() == 1 { "" } else { "s" },
                    types::format_size(total_size),
                ));
                println!();
                for b in &local_backups {
                    output::print_info(&format!(
                        "🔒 {}  {}  {}",
                        b.filename,
                        types::format_size(b.size),
                        format_system_time(b.modified),
                    ));
                }
            }

            // Cloud backups section
            output::print_section("Cloud Backups");
            if cloud_backups.is_empty() {
                output::print_info("No cloud backups found.");
            } else {
                for b in &cloud_backups {
                    let time_str = types::format_time(&b.created_at);
                    output::print_info(&format!(
                        "🔒 {}  {}  {}",
                        b.filename,
                        types::format_size(b.size),
                        time_str,
                    ));
                }
            }

            if local_backups.is_empty() && cloud_backups.is_empty() {
                println!();
                output::print_info("No backups found. Run 'calmbackup run' to create your first backup.");
            }
        }
    }

    Ok(())
}
