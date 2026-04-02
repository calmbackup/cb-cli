use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::core::types::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub encryption_key: String,
    #[serde(default = "default_api_url")]
    pub api_url: String,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub local_path: String,
    #[serde(default = "default_retention_days")]
    pub local_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
    pub path: Option<String>,
}

fn default_api_url() -> String {
    "https://app.calmbackup.com/api/v1".to_string()
}

fn default_retention_days() -> u32 {
    7
}

impl Config {
    /// Load and validate config from a specific path.
    pub fn load(path: &Path) -> Result<Self> {
        todo!("Load YAML config from path, validate required fields")
    }

    /// Load config without full validation (for partial use like notifications).
    pub fn load_partial(path: &Path) -> Result<Self> {
        todo!("Load YAML config without validation")
    }

    /// Find config file by searching standard locations.
    /// Order: --config flag > /etc/calmbackup/calmbackup.yaml > ~/.config/calmbackup/calmbackup.yaml > ./calmbackup.yaml
    pub fn find_config_file() -> Option<PathBuf> {
        todo!("Search standard config locations")
    }

    /// Return the config directory based on effective user (root vs normal).
    pub fn config_dir() -> PathBuf {
        todo!("Return /etc/calmbackup or ~/.config/calmbackup")
    }

    /// Return the local backup storage directory.
    pub fn local_path_default() -> PathBuf {
        todo!("Return /var/backups/calmbackup or ~/.local/share/calmbackup")
    }

    /// Validate all required fields are present and valid.
    fn validate(&self) -> Result<()> {
        todo!("Validate api_key, encryption_key, database.driver")
    }
}
