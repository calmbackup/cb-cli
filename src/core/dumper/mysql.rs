use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::Result;

pub struct MysqlDumper {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

impl MysqlDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        todo!("Extract MySQL connection params from config")
    }
}

impl DatabaseDumper for MysqlDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        todo!("Run mysqldump --single-transaction --routines --triggers")
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        todo!("Check dump contains '-- Dump completed'")
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        todo!("Pipe dump to mysql CLI")
    }

    fn filename(&self) -> &str {
        "database.sql"
    }
}
