use super::{
    api::ApiClient,
    archive,
    config::{Config, DatabaseConfig},
    crypto, dumper,
    restore::RestoreService,
};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

// Use the real SQLite dumper/restore and a loopback-only cloud endpoint. These
// tests prove checksum/authentication gates run before touching the destination.
#[tokio::test]
async fn restores_sqlite_only_after_checksum_and_authentication_pass() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    let db = rusqlite::Connection::open(&source).unwrap();
    db.execute_batch("CREATE TABLE people(id INTEGER PRIMARY KEY, name BLOB); INSERT INTO people VALUES (1, X'00ff0161'), (2, NULL);").unwrap();
    drop(db);
    let db_config = |path: &std::path::Path| DatabaseConfig {
        driver: "sqlite".into(),
        path: Some(path.to_string_lossy().into()),
        host: None,
        port: None,
        username: None,
        password: None,
        database: None,
        databases: Vec::new(),
    };
    let dump = dir.path().join("database.sqlite");
    let source_dumper = dumper::new_dumper(&db_config(&source)).unwrap();
    // Do not assume the dumper filename; it is part of the real restore contract.
    let dump = dump.with_file_name(source_dumper.filename());
    source_dumper.dump(&dump).unwrap();
    assert!(source_dumper.verify(&dump).unwrap());
    let archive_path = dir.path().join("archive.tar.gz");
    let encrypted = dir.path().join("backup.enc");
    let key = crypto::derive_key("sqlite-integration-test");
    archive::create(&dump, &[], &archive_path).unwrap();
    crypto::encrypt(&archive_path, &encrypted, &key).unwrap();
    let bytes = std::fs::read(&encrypted).unwrap();
    let checksum = crypto::checksum(&encrypted).unwrap();

    for mode in ["checksum-mismatch", "wrong-key", "tampered", "success"] {
        let target = dir.path().join(format!("{mode}.sqlite"));
        let db = rusqlite::Connection::open(&target).unwrap();
        db.execute_batch("CREATE TABLE sentinel(id INTEGER); INSERT INTO sentinel VALUES(42);")
            .unwrap();
        drop(db);
        let before = std::fs::read(&target).unwrap();
        let cache = dir.path().join(format!("{mode}-cache"));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut payload = bytes.clone();
        if mode == "tampered" {
            let last = payload.len() - 1;
            payload[last] ^= 1;
        }
        // For tampering, supply the hash of the damaged file to exercise the
        // GCM gate rather than stopping at the independent checksum gate.
        let expected = match mode {
            "checksum-mismatch" => "0".repeat(64),
            "tampered" => {
                use sha2::Digest;
                hex::encode(sha2::Sha256::digest(&payload))
            }
            _ => checksum.clone(),
        };
        let metadata = serde_json::to_vec(&serde_json::json!({
            "id":"test-id", "filename":"backup.enc", "size":payload.len(),
            "checksum":expected, "created_at":"2026-09-14T00:00:00Z", "download_url":format!("{url}/object"),
        })).unwrap();
        let server = std::thread::spawn(move || {
            for response in [metadata, payload] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                }
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",response.len()).unwrap();
                socket.write_all(&response).unwrap();
            }
        });
        let config = Config {
            api_key: "test".into(),
            encryption_key: "test".into(),
            api_url: url.clone(),
            database: db_config(&target),
            directories: vec![],
            local_path: cache.to_string_lossy().into(),
            local_retention_days: 7,
        };
        let service = RestoreService {
            dumper: dumper::new_dumper(&config.database).unwrap(),
            api: ApiClient::new("test", &url, "test"),
            config,
            key: if mode == "wrong-key" {
                crypto::derive_key("wrong")
            } else {
                key
            },
        };
        let result = service.restore("test-id", false, Box::new(|_, _| {})).await;
        server.join().unwrap();
        if mode == "success" {
            result.unwrap();
            assert_eq!(
                std::fs::read(&target).unwrap(),
                std::fs::read(&dump).unwrap()
            );
            let db = rusqlite::Connection::open(&target).unwrap();
            assert_eq!(
                db.query_row("SELECT hex(name) FROM people WHERE id=1", [], |row| row
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
                "00FF0161"
            );
            assert_eq!(
                db.query_row(
                    "SELECT COUNT(*) FROM people WHERE name IS NULL",
                    [],
                    |row| row.get::<_, u64>(0)
                )
                .unwrap(),
                1
            );
        } else {
            assert!(result.is_err(), "{mode} unexpectedly restored");
            assert_eq!(
                std::fs::read(&target).unwrap(),
                before,
                "{mode} changed database"
            );
        }
    }
}
