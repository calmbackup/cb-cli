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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: write YAML content to a temp file and return its path.
    fn write_temp_yaml(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("cb_test_{}", name));
        let mut f = std::fs::File::create(&path).expect("create temp file");
        f.write_all(content.as_bytes()).expect("write temp file");
        path
    }

    #[test]
    fn load_valid_config() {
        let yaml = r#"
api_key: "test-key-123"
encryption_key: "enc-key-456"
api_url: "https://custom.api/v1"
local_path: "/tmp/backups"
local_retention_days: 14
database:
  driver: mysql
  host: localhost
  port: 3306
  username: root
  password: secret
  database: mydb
directories:
  - /var/www
  - /etc/nginx
"#;
        let path = write_temp_yaml("valid.yaml", yaml);
        let config = Config::load(&path).expect("should load valid config");
        std::fs::remove_file(&path).ok();

        assert_eq!(config.api_key, "test-key-123");
        assert_eq!(config.encryption_key, "enc-key-456");
        assert_eq!(config.api_url, "https://custom.api/v1");
        assert_eq!(config.local_path, "/tmp/backups");
        assert_eq!(config.local_retention_days, 14);
        assert_eq!(config.database.driver, "mysql");
        assert_eq!(config.database.host.as_deref(), Some("localhost"));
        assert_eq!(config.database.port, Some(3306));
        assert_eq!(config.database.username.as_deref(), Some("root"));
        assert_eq!(config.database.password.as_deref(), Some("secret"));
        assert_eq!(config.database.database.as_deref(), Some("mydb"));
        assert_eq!(config.directories, vec!["/var/www", "/etc/nginx"]);
    }

    #[test]
    fn load_minimal_config_applies_defaults() {
        let yaml = r#"
api_key: "key"
encryption_key: "enc"
database:
  driver: sqlite
"#;
        let path = write_temp_yaml("minimal.yaml", yaml);
        let config = Config::load(&path).expect("should load minimal config");
        std::fs::remove_file(&path).ok();

        assert_eq!(config.api_url, "https://app.calmbackup.com/api/v1");
        assert_eq!(config.local_retention_days, 7);
        assert!(config.directories.is_empty());
    }

    #[test]
    fn load_missing_api_key() {
        let yaml = r#"
api_key: ""
encryption_key: "enc"
database:
  driver: mysql
"#;
        let path = write_temp_yaml("no_api_key.yaml", yaml);
        let err = Config::load(&path).unwrap_err();
        std::fs::remove_file(&path).ok();

        match err {
            AppError::Config(msg) => assert!(msg.contains("api_key"), "msg: {}", msg),
            other => panic!("expected AppError::Config, got: {:?}", other),
        }
    }

    #[test]
    fn load_missing_encryption_key() {
        let yaml = r#"
api_key: "key"
encryption_key: ""
database:
  driver: mysql
"#;
        let path = write_temp_yaml("no_enc_key.yaml", yaml);
        let err = Config::load(&path).unwrap_err();
        std::fs::remove_file(&path).ok();

        match err {
            AppError::Config(msg) => assert!(msg.contains("encryption_key"), "msg: {}", msg),
            other => panic!("expected AppError::Config, got: {:?}", other),
        }
    }

    #[test]
    fn load_missing_database_driver() {
        let yaml = r#"
api_key: "key"
encryption_key: "enc"
database:
  host: localhost
"#;
        let path = write_temp_yaml("no_driver.yaml", yaml);
        let err = Config::load(&path).unwrap_err();
        std::fs::remove_file(&path).ok();

        match err {
            AppError::Config(msg) => assert!(msg.contains("parse"), "msg: {}", msg),
            other => panic!("expected AppError::Config parse error, got: {:?}", other),
        }
    }

    #[test]
    fn load_invalid_driver_value() {
        let yaml = r#"
api_key: "key"
encryption_key: "enc"
database:
  driver: mongodb
"#;
        let path = write_temp_yaml("bad_driver.yaml", yaml);
        let err = Config::load(&path).unwrap_err();
        std::fs::remove_file(&path).ok();

        match err {
            AppError::Config(msg) => assert!(msg.contains("mongodb"), "msg: {}", msg),
            other => panic!("expected AppError::Config, got: {:?}", other),
        }
    }

    #[test]
    fn load_empty_local_path_gets_default() {
        let yaml = r#"
api_key: "key"
encryption_key: "enc"
database:
  driver: pgsql
"#;
        let path = write_temp_yaml("empty_local.yaml", yaml);
        let config = Config::load(&path).expect("should load");
        std::fs::remove_file(&path).ok();

        // local_path should be auto-populated, not empty
        assert!(!config.local_path.is_empty(), "local_path should not be empty");
        let default = Config::local_path_default().to_string_lossy().to_string();
        assert_eq!(config.local_path, default);
    }

    #[test]
    fn load_partial_skips_validation() {
        let yaml = r#"
api_key: ""
encryption_key: ""
database:
  driver: mysql
"#;
        let path = write_temp_yaml("partial.yaml", yaml);
        let config = Config::load_partial(&path);
        std::fs::remove_file(&path).ok();

        assert!(config.is_ok(), "load_partial should succeed without validation");
    }

    #[test]
    fn find_config_file_finds_local() {
        // Write a config at ./calmbackup.yaml relative to CWD
        let local = PathBuf::from("./calmbackup.yaml");
        let existed = local.exists();
        if !existed {
            std::fs::write(&local, "api_key: test\n").expect("write local config");
        }
        let found = Config::find_config_file();
        if !existed {
            std::fs::remove_file(&local).ok();
        }
        assert!(found.is_some(), "should find local config file");
    }

    #[test]
    fn config_dir_returns_non_empty() {
        let dir = Config::config_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn local_path_default_returns_non_empty() {
        let dir = Config::local_path_default();
        assert!(!dir.as_os_str().is_empty());
    }
}
