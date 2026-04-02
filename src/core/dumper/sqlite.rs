use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::Result;

pub struct SqliteDumper {
    db_path: String,
}

impl SqliteDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        todo!("Extract SQLite path from config")
    }
}

impl DatabaseDumper for SqliteDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        todo!("Try sqlite3 .backup command, fallback to file copy")
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        todo!("Run PRAGMA integrity_check, return true if output is 'ok'")
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        todo!("Direct file copy (SQLite has no restore CLI)")
    }

    fn filename(&self) -> &str {
        "database.sqlite"
    }
}
