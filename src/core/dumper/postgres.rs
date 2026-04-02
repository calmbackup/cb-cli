use std::path::Path;
use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::Result;

pub struct PostgresDumper {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

impl PostgresDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        todo!("Extract PostgreSQL connection params from config")
    }
}

impl DatabaseDumper for PostgresDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        todo!("Run pg_dump --format=custom with PGPASSWORD env var")
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        todo!("Run pg_restore --list, return true if exit code 0")
    }

    fn restore(&self, dump_path: &Path) -> Result<()> {
        todo!("Run pg_restore --clean --if-exists with PGPASSWORD env var")
    }

    fn filename(&self) -> &str {
        "database.pgdump"
    }
}
