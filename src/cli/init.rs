use crate::cli::output;
use crate::core::config::{Config, DatabaseConfig};
use crate::core::types::{AppError, Result};

/// Execute the interactive init wizard.
/// Prompts for API key, DB driver, connection params, generates encryption key.
pub async fn execute() -> Result<()> {
    output::print_header();

    output::print_section("Setup");

    // Prompt for API key
    let api_key = output::prompt("API key: ")?;
    if api_key.is_empty() {
        return Err(AppError::Config("API key is required.".into()));
    }

    // Prompt for database driver
    println!();
    output::print_info("Database driver:");
    output::print_info("  1. MySQL");
    output::print_info("  2. PostgreSQL");
    output::print_info("  3. SQLite");
    println!();
    let driver_choice = output::prompt("Select driver (1-3): ")?;

    let driver = match driver_choice.as_str() {
        "1" => "mysql",
        "2" => "pgsql",
        "3" => "sqlite",
        _ => return Err(AppError::Config("Invalid driver selection.".into())),
    };

    // Prompt for connection params based on driver
    let db_config = if driver == "sqlite" {
        let path = output::prompt("Database path: ")?;
        if path.is_empty() {
            return Err(AppError::Config("Database path is required.".into()));
        }
        DatabaseConfig {
            driver: driver.to_string(),
            host: None,
            port: None,
            username: None,
            password: None,
            database: None,
            path: Some(path),
        }
    } else {
        println!();
        let default_port = if driver == "mysql" { "3306" } else { "5432" };
        let host = output::prompt("Host (default: 127.0.0.1): ")?;
        let host = if host.is_empty() { "127.0.0.1".to_string() } else { host };

        let port_str = output::prompt(&format!("Port (default: {default_port}): "))?;
        let port: u16 = if port_str.is_empty() {
            default_port.parse().unwrap()
        } else {
            port_str.parse().map_err(|_| AppError::Config("Invalid port number.".into()))?
        };

        let username = output::prompt("Username: ")?;
        let password = output::prompt("Password: ")?;
        let database = output::prompt("Database name: ")?;

        if database.is_empty() {
            return Err(AppError::Config("Database name is required.".into()));
        }

        DatabaseConfig {
            driver: driver.to_string(),
            host: Some(host),
            port: Some(port),
            username: Some(username),
            password: Some(password),
            database: Some(database),
            path: None,
        }
    };

    // Generate random 32-byte encryption key
    let encryption_key = hex::encode(rand::random::<[u8; 32]>());

    // Build config
    let config_dir = Config::config_dir();
    let local_path = Config::local_path_default();

    let config = Config {
        api_key,
        encryption_key: encryption_key.clone(),
        api_url: "https://app.calmbackup.com/api/v1".to_string(),
        database: db_config,
        directories: Vec::new(),
        local_path: local_path.to_string_lossy().to_string(),
        local_retention_days: 7,
    };

    // Create directories
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(&local_path)?;

    // Write config YAML
    let config_path = config_dir.join("calmbackup.yaml");
    let yaml = serde_yaml::to_string(&config)
        .map_err(|e| AppError::Config(format!("Failed to serialize config: {}", e)))?;
    std::fs::write(&config_path, &yaml)?;

    // Write recovery key file
    let recovery_key_path = config_dir.join("calmbackup-recovery-key.txt");
    let recovery_content = format!(
        "CALM BACKUP RECOVERY KEY\n\
         ========================\n\
         \n\
         Store this key in a safe place. You will need it to restore\n\
         your backups if you lose access to your configuration.\n\
         \n\
         Recovery Key: {}\n\
         \n\
         Generated: {}\n",
        encryption_key,
        chrono::Local::now().format("%b %-d, %Y %H:%M"),
    );
    std::fs::write(&recovery_key_path, &recovery_content)?;

    // Print success
    output::print_success("Configuration saved");
    output::print_label("Config", &config_path.to_string_lossy());
    output::print_label("Recovery key", &recovery_key_path.to_string_lossy());
    output::print_label("Local backups", &local_path.to_string_lossy());
    println!();
    output::print_info("Save your recovery key in a secure location!");
    println!();

    Ok(())
}
