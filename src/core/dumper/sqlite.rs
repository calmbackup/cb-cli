use std::path::Path;
use std::time::Duration;

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::core::config::DatabaseConfig;
use crate::core::dumper::DatabaseDumper;
use crate::core::types::{AppError, Result};

/// How long to wait for a writer's lock before giving up on a step.
const BUSY_TIMEOUT: Duration = Duration::from_secs(1);
/// Pages copied per step. A read transaction pins the source snapshot, so
/// writers cannot restart this progress between chunks.
const PAGES_PER_STEP: i32 = 100;
/// Pause before retrying when the source is busy/locked.
const STEP_PAUSE: Duration = Duration::from_millis(250);
/// Overall deadline for producing a SQLite snapshot. This bounds retries when
/// a write-heavy database continually invalidates the online backup snapshot.
const BACKUP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

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
    /// Produce a consistent snapshot of the (possibly live) database.
    ///
    /// Uses SQLite's online backup API — the only correct way to copy a
    /// database that may be written concurrently. Unlike a raw file copy it is
    /// transactionally consistent and handles WAL mode correctly. We never fall
    /// back to copying the file: a failed dump must surface as a clean,
    /// retryable error, never a silently-corrupt backup that only gets caught
    /// (or worse, missed) downstream.
    fn dump(&self, output_path: &Path) -> Result<()> {
        let started = std::time::Instant::now();
        let src = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::Dump(format!("failed to open source database: {}", e)))?;
        // Wait for a concurrent writer's lock instead of failing immediately.
        src.busy_timeout(BUSY_TIMEOUT)
            .map_err(|e| AppError::Dump(format!("failed to set busy_timeout: {}", e)))?;
        // Pin a consistent source snapshot before starting the backup. Without
        // this explicit read transaction, every external commit can restart
        // sqlite3_backup and starve a busy database forever.
        loop {
            match src.execute_batch("BEGIN; SELECT name FROM sqlite_schema LIMIT 1;") {
                Ok(()) => break,
                Err(e)
                    if matches!(
                        e.sqlite_error_code(),
                        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
                    ) =>
                {
                    let _ = src.execute_batch("ROLLBACK");

                    if started.elapsed() >= BACKUP_TIMEOUT {
                        return Err(AppError::Dump(format!(
                            "online backup timed out after {} seconds; database remained busy",
                            BACKUP_TIMEOUT.as_secs()
                        )));
                    }
                }
                Err(e) => {
                    return Err(AppError::Dump(format!(
                        "failed to start snapshot transaction: {}",
                        e
                    )));
                }
            }
        }

        let mut dst = Connection::open(output_path)
            .map_err(|e| AppError::Dump(format!("failed to create dump file: {}", e)))?;

        let backup = Backup::new(&src, &mut dst)
            .map_err(|e| AppError::Dump(format!("failed to initialise backup: {}", e)))?;
        let remaining = BACKUP_TIMEOUT.saturating_sub(started.elapsed());
        run_to_completion_with_timeout(&backup, remaining, STEP_PAUSE)?;

        Ok(())
    }

    fn verify(&self, dump_path: &Path) -> Result<bool> {
        let conn = Connection::open_with_flags(dump_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::DumpVerify(format!("failed to open dump: {}", e)))?;
        let result: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| AppError::DumpVerify(format!("integrity_check failed: {}", e)))?;
        Ok(result == "ok")
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

/// Copy the database in small chunks while enforcing a real wall-clock
/// deadline. rusqlite's `run_to_completion` retries BUSY/LOCKED forever, which
/// allowed one cron process per day to accumulate without ever reaching the
/// failure-notification path.
fn run_to_completion_with_timeout(
    backup: &Backup<'_, '_>,
    timeout: Duration,
    pause: Duration,
) -> Result<()> {
    let started = std::time::Instant::now();

    loop {
        let step_result = backup
            .step(PAGES_PER_STEP)
            .map_err(|e| AppError::Dump(format!("online backup failed: {}", e)))?;

        match step_result {
            StepResult::Done => return Ok(()),
            StepResult::More | StepResult::Busy | StepResult::Locked => {
                if started.elapsed() >= timeout {
                    return Err(AppError::Dump(format!(
                        "online backup timed out after {} seconds; database remained busy",
                        timeout.as_secs()
                    )));
                }

                if matches!(step_result, StepResult::Busy | StepResult::Locked) {
                    std::thread::sleep(pause);
                }
            }
            _ => unreachable!("rusqlite returned an unknown backup step result"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn test_dir() -> PathBuf {
        let d = std::env::temp_dir().join("calmbackup_sqlite_tests");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_db(path: &Path, rows: usize) {
        let _ = std::fs::remove_file(path);
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT);")
            .unwrap();
        for i in 0..rows {
            conn.execute("INSERT INTO t(v) VALUES(?1)", params![format!("row-{i}")])
                .unwrap();
        }
    }

    fn dumper_for(path: &Path) -> SqliteDumper {
        SqliteDumper {
            db_path: path.to_string_lossy().into_owned(),
        }
    }

    fn row_count(path: &Path) -> i64 {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn dump_then_verify_is_valid_and_complete() {
        let dir = test_dir();
        let db = dir.join("basic.sqlite");
        let out = dir.join("basic-out.sqlite");
        make_db(&db, 500);

        let d = dumper_for(&db);
        d.dump(&out).unwrap();

        assert!(d.verify(&out).unwrap(), "a fresh dump must pass integrity_check");
        assert_eq!(row_count(&out), 500, "dump must contain every row");

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn verify_rejects_a_non_database_file() {
        let dir = test_dir();
        let bad = dir.join("garbage.sqlite");
        std::fs::write(&bad, b"this is not a sqlite database at all").unwrap();

        let d = dumper_for(&bad);
        // A torn/corrupt dump must never be reported as valid — Ok(false) or Err
        // are both acceptable, Ok(true) is the bug.
        assert!(
            !matches!(d.verify(&bad), Ok(true)),
            "a non-database file must not verify as valid"
        );

        let _ = std::fs::remove_file(&bad);
    }

    /// Regression test for the "Dump verification failed" outage: the old code
    /// fell back to a raw file copy of a live database under lock contention,
    /// producing a corrupt dump. The online backup API must stay internally
    /// consistent no matter how hard the database is being written.
    #[test]
    fn dump_stays_consistent_under_concurrent_writes() {
        let dir = test_dir();
        let db = dir.join("concurrent.sqlite");
        make_db(&db, 100);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_writer = stop.clone();
        let db_writer = db.clone();

        // A storm of independent transactions — each COMMIT briefly takes an
        // EXCLUSIVE lock, the exact contention that broke the old fallback.
        let writer = std::thread::spawn(move || {
            let conn = Connection::open(&db_writer).unwrap();
            conn.busy_timeout(BUSY_TIMEOUT).unwrap();
            let mut i = 0i64;
            while !stop_writer.load(Ordering::Relaxed) {
                conn.execute("INSERT INTO t(v) VALUES(?1)", params![format!("w-{i}")])
                    .unwrap();
                i += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        let d = dumper_for(&db);
        for iter in 0..5 {
            let out = dir.join(format!("concurrent-out-{iter}.sqlite"));
            d.dump(&out).expect("dump under write load must not error");
            assert!(
                d.verify(&out).unwrap(),
                "dump #{iter} taken under write load must pass integrity_check"
            );
            let _ = std::fs::remove_file(&out);
        }

        stop.store(true, Ordering::Relaxed);
        writer.join().unwrap();
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn busy_database_respects_overall_backup_deadline() {
        let dir = test_dir();
        let db = dir.join("locked.sqlite");
        let out = dir.join("locked-out.sqlite");
        make_db(&db, 10);

        let locker = Connection::open(&db).unwrap();
        locker.execute_batch("BEGIN EXCLUSIVE; INSERT INTO t(v) VALUES('locked');").unwrap();

        let src = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut dst = Connection::open(&out).unwrap();
        let backup = Backup::new(&src, &mut dst).unwrap();
        let started = std::time::Instant::now();
        let timeout = Duration::from_millis(50);

        let error = run_to_completion_with_timeout(
            &backup,
            timeout,
            Duration::from_millis(5),
        )
        .expect_err("a permanently locked database must time out");

        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() >= timeout);

        drop(backup);
        locker.execute_batch("ROLLBACK").unwrap();
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&out);
    }
}
