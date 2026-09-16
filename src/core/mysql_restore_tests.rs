//! Explicit opt-in integration test against two empty, isolated MySQL servers.
use super::config::DatabaseConfig;
use super::dumper::{DatabaseDumper, mysql::MysqlDumper};
use std::process::{Command, Stdio};
use std::io::Write;

fn query(port: u16, sql: &str) -> String {
    let mut child = Command::new("mysql")
        .args(["-h127.0.0.1", &format!("-P{port}"), "-uroot", "--batch", "--raw",
               "--skip-column-names", "--default-character-set=utf8mb4"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("test mysql client");
    child.stdin.take().unwrap().write_all(sql.as_bytes()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "synthetic SQL failed: {}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout).unwrap()
}

#[test]
#[ignore = "requires two fresh isolated MySQL servers; never run against application databases"]
fn mysql_multi_database_snapshot_restores_original_schemas_and_values() {
    require_empty_pair();
    let names = ["cb_fixture_api", "cb_fixture_spine", "cb_fixture_auth"];
    for name in names {
        query(3306, &format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
            CREATE TABLE `{name}`.sample (id INT PRIMARY KEY, name TEXT NULL, payload BLOB NULL) ENGINE=InnoDB;
            INSERT INTO `{name}`.sample VALUES (1,'Zoë — 東京',X'00FF0102'),(2,NULL,NULL),(3,'quotes; and apostrophe''s','');"));
    }
    roundtrip_fixture(names);
}

fn require_empty_pair() {
    assert_eq!(std::env::var("CB_MYSQL_FIXTURE").unwrap(), "two-empty-isolated-servers");
    for (port, name) in [(3306, "CB_MYSQL_SOURCE_UUID"), (3307, "CB_MYSQL_RESTORE_UUID")] {
        let expected = std::env::var(name).unwrap();
        assert_eq!(expected.len(), 36);
        assert_ne!(expected, "78150142-17fe-11f1-933f-da046753b8dc", "production forbidden");
        assert_eq!(query(port, "SELECT @@server_uuid;").trim(), expected);
        assert_eq!(query(port, "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME NOT IN ('mysql','information_schema','performance_schema','sys');").trim(), "0",
                   "empty synthetic server required; no existing database may be overwritten");
        assert_eq!(query(port, "SELECT @@event_scheduler;").trim(), "OFF");
    }
    assert_ne!(std::env::var("CB_MYSQL_SOURCE_UUID").unwrap(), std::env::var("CB_MYSQL_RESTORE_UUID").unwrap());
}

fn configuration(port: u16, names: &[&str]) -> DatabaseConfig {
    DatabaseConfig {
        driver: "mysql".into(), host: Some("127.0.0.1".into()), port: Some(port),
        username: Some("root".into()), password: Some(String::new()), database: None,
        databases: names.iter().map(|s| s.to_string()).collect(), path: None,
    }
}

fn roundtrip_fixture(names: [&str; 3]) {
    query(3306, "CREATE TABLE cb_fixture_spine.link (id INT PRIMARY KEY, FOREIGN KEY(id) REFERENCES cb_fixture_api.sample(id)) ENGINE=InnoDB; INSERT INTO cb_fixture_spine.link VALUES(1);
        CREATE VIEW cb_fixture_auth.names AS SELECT id,name FROM cb_fixture_auth.sample;
        CREATE PROCEDURE cb_fixture_auth.fixture_proc() SELECT 1;
        CREATE TRIGGER cb_fixture_api.fixture_trigger BEFORE INSERT ON cb_fixture_api.sample FOR EACH ROW SET NEW.name=COALESCE(NEW.name,'fixture');
        CREATE EVENT cb_fixture_auth.fixture_event ON SCHEDULE EVERY 1 DAY STARTS '2035-01-01 00:00:00' DO SELECT 1;");
    let temp = tempfile::tempdir().unwrap();
    let dump = temp.path().join("database.sql");
    let source = MysqlDumper::new(&configuration(3306, &names)).unwrap();
    source.dump(&dump).unwrap();
    assert!(source.verify(&dump).unwrap());
    MysqlDumper::new(&configuration(3307, &names)).unwrap().restore(&dump).unwrap();
    for name in names {
        let sql = format!("SELECT id,HEX(name),HEX(payload),name IS NULL,payload IS NULL FROM `{name}`.sample ORDER BY id;");
        assert_eq!(query(3306, &sql), query(3307, &sql));
        let sql = format!("SHOW CREATE DATABASE `{name}`;");
        assert_eq!(query(3306, &sql), query(3307, &sql));
        let sql = format!("SELECT COLUMN_NAME,ORDINAL_POSITION,COLUMN_TYPE,IS_NULLABLE,COLUMN_DEFAULT,CHARACTER_SET_NAME,COLLATION_NAME,EXTRA,COLUMN_COMMENT FROM information_schema.COLUMNS WHERE TABLE_SCHEMA='{name}' AND TABLE_NAME='sample' ORDER BY ORDINAL_POSITION;");
        assert_eq!(query(3306, &sql), query(3307, &sql));
        let sql = format!("SHOW CREATE TABLE `{name}`.sample;");
        // MySQL may display the inherited column charset explicitly after a
        // logical restore. Only this exact fixture clause is normalized; all
        // actual column metadata above and every other DDL byte must match.
        let display = |ddl: String| ddl.replace(
            "`name` text CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            "`name` text COLLATE utf8mb4_unicode_ci");
        assert_eq!(display(query(3306, &sql)), display(query(3307, &sql)));
    }
    assert_eq!(query(3307, "SELECT id FROM cb_fixture_spine.link;"), "1\n");
    assert_eq!(query(3306, "SHOW CREATE TABLE cb_fixture_spine.link;"),
               query(3307, "SHOW CREATE TABLE cb_fixture_spine.link;"));
    for sql in [
        "SELECT COUNT(*) FROM information_schema.VIEWS WHERE TABLE_SCHEMA='cb_fixture_auth';",
        "SELECT COUNT(*) FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA='cb_fixture_auth';",
        "SELECT COUNT(*) FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA='cb_fixture_api';",
        "SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA='cb_fixture_auth';",
    ] {
        assert_eq!(query(3307, sql), "1\n");
    }
    assert_eq!(query(3307, "CALL cb_fixture_auth.fixture_proc();"), "1\n");
    // All fixture databases are retained for inspection. No DROP/cleanup here.
}

#[test]
#[ignore = "requires two fresh isolated MySQL servers; synthetic concurrent writer only"]
fn mysql_joint_snapshot_is_consistent_during_cross_schema_transactions() {
    use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
    use std::time::{Duration, Instant};
    require_empty_pair();
    let names = ["cb_fixture_api", "cb_fixture_spine", "cb_fixture_auth"];
    for name in names {
        query(3306, &format!("CREATE DATABASE `{name}`;
            CREATE TABLE `{name}`.epoch (id INT PRIMARY KEY, revision BIGINT NOT NULL) ENGINE=InnoDB;
            INSERT INTO `{name}`.epoch VALUES(1,0);"));
    }
    // Enough first-schema data for the independently committed writer to make
    // progress while mysqldump traverses the three schemas. All synthetic.
    query(3306, "CREATE TABLE cb_fixture_api.payload (id INT PRIMARY KEY, body TEXT NOT NULL) ENGINE=InnoDB;
        SET SESSION cte_max_recursion_depth=10000;
        INSERT INTO cb_fixture_api.payload WITH RECURSIVE numbers AS
        (SELECT 1 AS n UNION ALL SELECT n+1 FROM numbers WHERE n<10000)
        SELECT n,REPEAT('x',4096) FROM numbers;");
    let stop = Arc::new(AtomicBool::new(false));
    let revision = Arc::new(AtomicU64::new(0));
    let (writer_stop, writer_revision) = (stop.clone(), revision.clone());
    let writer = std::thread::spawn(move || {
        let mut n = 0u64;
        while !writer_stop.load(Ordering::SeqCst) {
            n += 1;
            query(3306, &format!("START TRANSACTION;
                UPDATE cb_fixture_api.epoch SET revision={n};
                UPDATE cb_fixture_spine.epoch SET revision={n};
                UPDATE cb_fixture_auth.epoch SET revision={n}; COMMIT;"));
            writer_revision.store(n, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while revision.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let before = revision.load(Ordering::SeqCst);
    let temp = tempfile::tempdir().unwrap();
    let dump = temp.path().join("database.sql");
    let source = MysqlDumper::new(&configuration(3306, &names)).unwrap();
    let result = source.dump(&dump);
    let during = revision.load(Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);
    writer.join().unwrap();
    let after = revision.load(Ordering::SeqCst);
    result.unwrap();
    assert!(before >= 2 && during > before, "writer must demonstrably commit while dump runs");
    assert!(source.verify(&dump).unwrap());
    MysqlDumper::new(&configuration(3307, &names)).unwrap().restore(&dump).unwrap();
    let restored: Vec<u64> = names.iter().map(|name| {
        query(3307, &format!("SELECT revision FROM `{name}`.epoch WHERE id=1;")).trim().parse().unwrap()
    }).collect();
    assert!(restored.iter().all(|n| *n == restored[0]), "cross-schema recovery point differs");
    assert!(restored[0] >= before && restored[0] <= after);
    assert_eq!(query(3307, "SELECT COUNT(*),SUM(OCTET_LENGTH(body)) FROM cb_fixture_api.payload;"),
               "10000\t40960000\n");
    println!("Concurrent snapshot passed: before={before}, during={during}, after={after}, restored={}", restored[0]);
}
