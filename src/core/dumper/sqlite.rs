use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};

pub struct SqliteDumper {
    db_path: String,
}

impl SqliteDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        let db_path = config
            .path
            .clone()
            .ok_or_else(|| AppError::Config("SQLite path is required".to_string()))?;

        Ok(Self { db_path })
    }
}

impl DatabaseDumper for SqliteDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        use std::process::Command;

        let output_str = output_path.to_string_lossy();

        // Try sqlite3 .backup command first
        let result = Command::new("sqlite3")
            .arg(&self.db_path)
            .arg(format!(".backup '{}'", output_str))
            .output();

        match result {
            Ok(output) if output.status.success() => Ok(()),
            _ => {
                // Fallback to direct file copy
                std::fs::copy(&self.db_path, output_path)
                    .map_err(|e| AppError::Dump(format!("failed to copy SQLite database: {}", e)))?;
                Ok(())
            }
        }
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("sqlite3")
            .arg(dump_path)
            .arg("PRAGMA integrity_check;")
            .output()
            .map_err(|e| {
                AppError::DumpVerify(format!("failed to run sqlite3 integrity check: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("ok"))
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        std::fs::copy(dump_path, &self.db_path)
            .map_err(|e| AppError::Restore(format!("failed to copy SQLite database: {}", e)))?;
        Ok(())
    }

    fn filename(&self) -> &str {
        "database.sqlite"
    }
}
