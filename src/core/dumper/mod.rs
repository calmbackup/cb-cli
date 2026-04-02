pub mod mysql;
pub mod postgres;
pub mod sqlite;

use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::types::Result;

/// Trait for database dump/verify/restore operations.
pub trait DatabaseDumper: Send + Sync {
    /// Dump the database to the given output path.
    fn dump(&self, output_path: &Path) -> Result<()>;

    /// Verify a dump file is valid.
    fn verify(&self, dump_path: &Path) -> Result<bool>;

    /// Restore a database from a dump file.
    fn restore(&self, dump_path: &Path) -> Result<()>;

    /// Return the canonical dump filename (e.g., "database.sql").
    fn filename(&self) -> &str;
}

/// Create the appropriate dumper based on the database driver config.
pub fn new_dumper(config: &DatabaseConfig) -> Result<Box<dyn DatabaseDumper>> {
    match config.driver.as_str() {
        "mysql" => Ok(Box::new(mysql::MysqlDumper::new(config)?)),
        "pgsql" => Ok(Box::new(postgres::PostgresDumper::new(config)?)),
        "sqlite" => Ok(Box::new(sqlite::SqliteDumper::new(config)?)),
        other => Err(crate::core::types::AppError::Config(
            format!("Unknown database driver: {}", other),
        )),
    }
}
