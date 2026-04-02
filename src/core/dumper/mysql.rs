use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};

pub struct MysqlDumper {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

impl MysqlDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        let host = config
            .host
            .clone()
            .ok_or_else(|| AppError::Config("MySQL host is required".to_string()))?;
        let port = config
            .port
            .ok_or_else(|| AppError::Config("MySQL port is required".to_string()))?;
        let username = config
            .username
            .clone()
            .ok_or_else(|| AppError::Config("MySQL username is required".to_string()))?;
        let password = config
            .password
            .clone()
            .ok_or_else(|| AppError::Config("MySQL password is required".to_string()))?;
        let database = config
            .database
            .clone()
            .ok_or_else(|| AppError::Config("MySQL database is required".to_string()))?;

        Ok(Self {
            host,
            port,
            username,
            password,
            database,
        })
    }
}

impl DatabaseDumper for MysqlDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        use std::fs::File;
        use std::process::Command;

        let output_file = File::create(output_path)?;

        let mut cmd = Command::new("mysqldump");
        cmd.arg("--single-transaction")
            .arg("--routines")
            .arg("--triggers")
            .arg(format!("-h{}", self.host))
            .arg(format!("-P{}", self.port))
            .arg(format!("-u{}", self.username));
        if !self.password.is_empty() {
            cmd.arg(format!("-p{}", self.password));
        }
        let output = cmd
            .arg(&self.database)
            .stdout(output_file)
            .output()
            .map_err(|e| AppError::Dump(format!("failed to run mysqldump: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Dump(format!("mysqldump failed: {}", stderr)));
        }

        Ok(())
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        let contents = std::fs::read_to_string(dump_path)
            .map_err(|e| AppError::DumpVerify(format!("failed to read dump file: {}", e)))?;
        Ok(contents.contains("-- Dump completed"))
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        use std::fs::File;
        use std::process::Command;

        let input_file = File::open(dump_path)?;

        let mut cmd = Command::new("mysql");
        cmd.arg(format!("-h{}", self.host))
            .arg(format!("-P{}", self.port))
            .arg(format!("-u{}", self.username));
        if !self.password.is_empty() {
            cmd.arg(format!("-p{}", self.password));
        }
        let output = cmd
            .arg(&self.database)
            .stdin(input_file)
            .output()
            .map_err(|e| AppError::Restore(format!("failed to run mysql: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Restore(format!("mysql restore failed: {}", stderr)));
        }

        Ok(())
    }

    fn filename(&self) -> &str {
        "database.sql"
    }
}
