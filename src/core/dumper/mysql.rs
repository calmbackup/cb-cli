use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};
use std::path::Path;

pub struct MysqlDumper {
    host: String,
    port: u16,
    username: String,
    password: String,
    databases: Vec<String>,
    multi_database: bool,
}

impl MysqlDumper {
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        config.validate_selection()?;
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
        let multi_database = !config.databases.is_empty();
        let databases = if multi_database {
            config.databases.clone()
        } else {
            vec![config.database.clone().filter(|name| !name.is_empty())
                .ok_or_else(|| AppError::Config("MySQL database is required".to_string()))?]
        };

        Ok(Self {
            host,
            port,
            username,
            password,
            databases,
            multi_database,
        })
    }

    fn dump_command(&self) -> std::process::Command {
        let mut cmd = self.client_command("mysqldump");
        cmd.arg("--single-transaction").arg("--routines").arg("--triggers");
        if self.multi_database {
            // A single client transaction, not separate sequential dumps.
            // CREATE DATABASE/USE statements preserve the original schema names.
            cmd.arg("--events").arg("--databases");
        }
        cmd.arg("--").args(&self.databases);
        cmd
    }

    fn restore_command(&self) -> std::process::Command {
        let mut cmd = self.client_command("mysql");
        cmd.arg("--");
        if !self.multi_database {
            cmd.arg(&self.databases[0]);
        }
        cmd
    }

    fn client_command(&self, executable: &str) -> std::process::Command {
        let mut cmd = std::process::Command::new(executable);
        cmd.arg(format!("-h{}", self.host))
            .arg(format!("-P{}", self.port))
            .arg(format!("-u{}", self.username));
        if !self.password.is_empty() {
            cmd.arg(format!("-p{}", self.password));
        }
        cmd
    }
}

impl DatabaseDumper for MysqlDumper {
    fn dump(&self, output_path: &Path) -> Result<()> {
        use std::fs::File;

        let output_file = File::create(output_path)?;

        let output = self.dump_command()
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

        let input_file = File::open(dump_path)?;

        let output = self.restore_command()
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

    fn configured(selection: &str) -> DatabaseConfig {
        serde_yaml::from_str(&format!(
            "driver: mysql\nhost: localhost\nport: 3306\nusername: test\npassword: ''\n{selection}\n"
        )).unwrap()
    }

    fn arguments(command: std::process::Command) -> Vec<String> {
        command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn legacy_single_database_keeps_target_and_dump_format() {
        let dumper = MysqlDumper::new(&configured("database: example")).unwrap();
        let dump = arguments(dumper.dump_command());
        assert!(dump.contains(&"--single-transaction".into()));
        assert!(!dump.contains(&"--databases".into()));
        assert_eq!(&dump[dump.len()-2..], ["--", "example"]);
        let restore = arguments(dumper.restore_command());
        assert_eq!(&restore[restore.len()-2..], ["--", "example"]);
    }

    #[test]
    fn joint_snapshot_uses_one_transaction_and_explicit_schema_names() {
        let dumper = MysqlDumper::new(&configured("databases: [bb_api, bb_spine, keycloak]")).unwrap();
        let dump = arguments(dumper.dump_command());
        assert_eq!(dump.iter().filter(|arg| *arg == "--single-transaction").count(), 1);
        assert!(dump.contains(&"--events".into()));
        assert_eq!(&dump[dump.len()-5..], ["--databases", "--", "bb_api", "bb_spine", "keycloak"]);
        // USE statements select each original database, not a single forced target.
        assert_eq!(arguments(dumper.restore_command()), ["-hlocalhost", "-P3306", "-utest", "--"]);
    }

    #[test]
    fn invalid_ambiguous_or_duplicate_selection_is_rejected() {
        for selection in ["database: example\ndatabases: [other]", "databases: ['', example]",
                          "databases: [example, example]", "databases: [example, EXAMPLE]",
                          "databases: []", "database: ''"] {
            assert!(MysqlDumper::new(&configured(selection)).is_err(), "{selection}");
        }
        let mut config = configured("databases: [example]");
        config.driver = "pgsql".into();
        assert!(config.validate_selection().is_err());
    }

    #[test]
    fn database_names_cannot_be_interpreted_as_client_options() {
        let dumper = MysqlDumper::new(&configured("databases: ['--all-databases', 'a b']")).unwrap();
        let dump = arguments(dumper.dump_command());
        assert_eq!(&dump[dump.len()-3..], ["--", "--all-databases", "a b"]);
    }

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
