use std::path::PathBuf;
use thiserror::Error;

/// Progress callback for long-running operations.
/// Called with (step_description, optional_detail).
pub type ProgressFn = Box<dyn Fn(&str, Option<&str>) + Send + Sync>;

/// Result of a completed backup operation.
#[derive(Debug, Clone)]
pub struct BackupResult {
    pub filename: String,
    pub size: u64,
    pub duration: std::time::Duration,
    pub checksum: String,
}

/// Result of a completed restore operation.
#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub backup_id: String,
    pub filename: String,
    pub duration: std::time::Duration,
}

/// A backup entry (from cloud API or local filesystem).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupEntry {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub checksum: Option<String>,
    pub created_at: String,
    pub download_url: Option<String>,
}

/// Upload URL response from the API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadUrlResponse {
    pub upload_url: String,
    pub backup_id: String,
}

/// Account info from the API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccountInfo {
    pub backup_count: u64,
    pub storage_used: u64,
    pub last_backup_at: Option<String>,
}

/// All application errors.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database dump failed: {0}")]
    Dump(String),

    #[error("Dump verification failed: {0}")]
    DumpVerify(String),

    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Encryption error: {0}")]
    Crypto(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication failed (401)")]
    Authentication,

    #[error("Billing issue (402)")]
    Billing,

    #[error("Backup was deleted (409)")]
    BackupDeleted,

    #[error("Size limit exceeded (413)")]
    SizeLimit,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Rate limited (429)")]
    RateLimit,

    #[error("Server error: {0}")]
    Server(String),

    #[error("Upload error: {0}")]
    Upload(String),

    #[error("Download error: {0}")]
    Download(String),

    #[error("Restore error: {0}")]
    Restore(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;

/// Format bytes into human-readable size string.
pub fn format_size(bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;
    const KB: u64 = 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format an ISO 8601 timestamp into display format with relative time.
/// Output: "Jan 2, 2006 15:04 (Xd ago)"
pub fn format_time(iso_time: &str) -> String {
    use chrono::{DateTime, Utc};

    let parsed = iso_time
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());
    let now = Utc::now();
    let ago = now.signed_duration_since(parsed);

    let relative = if ago.num_days() > 60 {
        format!("{}mo ago", ago.num_days() / 30)
    } else if ago.num_days() > 0 {
        format!("{}d ago", ago.num_days())
    } else if ago.num_hours() > 0 {
        format!("{}h ago", ago.num_hours())
    } else if ago.num_minutes() > 0 {
        format!("{}m ago", ago.num_minutes())
    } else {
        "just now".to_string()
    };

    format!(
        "{} ({})",
        parsed.format("%b %-d, %Y %H:%M"),
        relative
    )
}

/// Local backup file info derived from the filesystem.
#[derive(Debug, Clone)]
pub struct LocalBackupInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: std::time::SystemTime,
}
