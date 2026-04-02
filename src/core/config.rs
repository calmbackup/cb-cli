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

fn is_root() -> bool {
    // Check effective UID via environment; fallback to checking USER env var
    std::env::var("EUID")
        .or_else(|_| std::env::var("UID"))
        .map(|uid| uid == "0")
        .unwrap_or_else(|_| {
            std::env::var("USER")
                .map(|u| u == "root")
                .unwrap_or(false)
        })
}

impl Config {
    /// Load and validate config from a specific path.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            AppError::Config(format!("Failed to read config file {}: {}", path.display(), e))
        })?;
        let mut config: Config = serde_yaml::from_str(&contents).map_err(|e| {
            AppError::Config(format!("Failed to parse config file: {}", e))
        })?;
        config.validate()?;
        if config.local_path.is_empty() {
            config.local_path = Self::local_path_default().to_string_lossy().to_string();
        }
        Ok(config)
    }

    /// Load config without full validation (for partial use like notifications).
    pub fn load_partial(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            AppError::Config(format!("Failed to read config file {}: {}", path.display(), e))
        })?;
        let mut config: Config = serde_yaml::from_str(&contents).map_err(|e| {
            AppError::Config(format!("Failed to parse config file: {}", e))
        })?;
        if config.local_path.is_empty() {
            config.local_path = Self::local_path_default().to_string_lossy().to_string();
        }
        Ok(config)
    }

    /// Find config file by searching standard locations.
    /// Order: /etc/calmbackup/calmbackup.yaml > ~/.config/calmbackup/calmbackup.yaml > ./calmbackup.yaml
    pub fn find_config_file() -> Option<PathBuf> {
        let candidates: Vec<Option<PathBuf>> = vec![
            Some(PathBuf::from("/etc/calmbackup/calmbackup.yaml")),
            dirs::home_dir()
                .map(|h| h.join(".config/calmbackup/calmbackup.yaml")),
            Some(PathBuf::from("./calmbackup.yaml")),
        ];
        for candidate in candidates {
            if let Some(path) = candidate {
                if path.exists() {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Return the config directory based on effective user (root vs normal).
    pub fn config_dir() -> PathBuf {
        if is_root() {
            PathBuf::from("/etc/calmbackup")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".config/calmbackup")
        }
    }

    /// Return the local backup storage directory.
    pub fn local_path_default() -> PathBuf {
        if is_root() {
            PathBuf::from("/var/backups/calmbackup")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".local/share/calmbackup")
        }
    }

    /// Validate all required fields are present and valid.
    fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(AppError::Config("api_key is required".to_string()));
        }
        if self.encryption_key.is_empty() {
            return Err(AppError::Config("encryption_key is required".to_string()));
        }
        let valid_drivers = ["mysql", "pgsql", "sqlite"];
        if !valid_drivers.contains(&self.database.driver.as_str()) {
            return Err(AppError::Config(format!(
                "Invalid database driver '{}'. Must be one of: mysql, pgsql, sqlite",
                self.database.driver
            )));
        }
        Ok(())
    }
}
