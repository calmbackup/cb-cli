use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};

pub struct PostgresDumper {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

impl PostgresDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        let host = config
            .host
            .clone()
            .ok_or_else(|| AppError::Config("PostgreSQL host is required".to_string()))?;
        let port = config
            .port
            .ok_or_else(|| AppError::Config("PostgreSQL port is required".to_string()))?;
        let username = config
            .username
            .clone()
            .ok_or_else(|| AppError::Config("PostgreSQL username is required".to_string()))?;
        let password = config
            .password
            .clone()
            .ok_or_else(|| AppError::Config("PostgreSQL password is required".to_string()))?;
        let database = config
            .database
            .clone()
            .ok_or_else(|| AppError::Config("PostgreSQL database is required".to_string()))?;

        Ok(Self {
            host,
            port,
            username,
            password,
            database,
        })
    }
}

impl DatabaseDumper for PostgresDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        use std::fs::File;
        use std::process::Command;

        let output_file = File::create(output_path)?;

        let output = Command::new("pg_dump")
            .arg("--format=custom")
            .arg("-h")
            .arg(&self.host)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-U")
            .arg(&self.username)
            .arg("-d")
            .arg(&self.database)
            .env("PGPASSWORD", &self.password)
            .stdout(output_file)
            .output()
            .map_err(|e| AppError::Dump(format!("failed to run pg_dump: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Dump(format!("pg_dump failed: {}", stderr)));
        }

        Ok(())
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("pg_restore")
            .arg("--list")
            .arg(dump_path)
            .output()
            .map_err(|e| AppError::DumpVerify(format!("failed to run pg_restore --list: {}", e)))?;

        Ok(output.status.success())
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        use std::process::Command;

        let output = Command::new("pg_restore")
            .arg("--clean")
            .arg("--if-exists")
            .arg("-h")
            .arg(&self.host)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-U")
            .arg(&self.username)
            .arg("-d")
            .arg(&self.database)
            .arg(dump_path)
            .env("PGPASSWORD", &self.password)
            .output()
            .map_err(|e| AppError::Restore(format!("failed to run pg_restore: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Restore(format!("pg_restore failed: {}", stderr)));
        }

        Ok(())
    }

    fn filename(&self) -> &str {
        "database.pgdump"
    }
}
