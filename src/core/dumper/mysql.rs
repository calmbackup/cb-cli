use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};
use std::path::Path;

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
        verify_dump(dump_path)
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
            return Err(AppError::Restore(format!(
                "mysql restore failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn filename(&self) -> &str {
        "database.sql"
    }
}

/// Inspect the bounded completion trailer as bytes, not an unbounded SQL line.
/// SQL BLOB content need not be UTF-8; mysqldump's final comment is ASCII.
pub fn verify_dump(path: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)
        .map_err(|e| AppError::DumpVerify(format!("failed to open dump: {e}")))?;
    let count = file.metadata()?.len().min(4096) as usize;
    file.seek(SeekFrom::End(-(count as i64)))?;
    let mut tail = [0; 4096];
    file.read_exact(&mut tail[..count])?;
    let line = tail[..count]
        .rsplit(|b| *b == b'\n')
        .find(|line| line.iter().any(|b| !b.is_ascii_whitespace()));
    Ok(line.is_some_and(|line| line.starts_with(b"-- Dump completed on ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn verifies_binary_dump_and_requires_completion_at_end() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"INSERT '\xff\xfe';\n-- Dump completed on 2026-09-14 12:00:00\r\n\n")
            .unwrap();
        assert!(verify_dump(file.path()).unwrap());
        file.write_all(b"INSERT incomplete").unwrap();
        assert!(!verify_dump(file.path()).unwrap());
    }

    #[test]
    fn rejects_empty_truncated_and_embedded_marker() {
        for data in [
            b"".as_slice(),
            b"-- Dump completed o",
            b"INSERT '-- Dump completed on fake';\n",
        ] {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(data).unwrap();
            assert!(!verify_dump(file.path()).unwrap());
        }
        assert!(verify_dump(Path::new("/nonexistent-calmbackup-dump.sql")).is_err());
    }

    #[test]
    fn verifies_24_gib_sparse_dump_without_loading_it() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(24 * 1024 * 1024 * 1024).unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(b"\n-- Dump completed on 2026-09-14 12:00:00\n")
            .unwrap();
        assert!(verify_dump(file.path()).unwrap());
    }
}
