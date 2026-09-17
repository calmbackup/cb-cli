//! Explicit recovery of one retained encrypted archive. Never opens a database,
//! exports data, prunes files, or retries a failed request automatically.
use super::{
    api::ApiClient,
    crypto,
    types::{AppError, BackupEntry, Result},
    upload,
};
use std::{collections::HashSet, path::Path};

#[derive(serde::Serialize)]
pub struct UploadReceipt {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub checksum: String,
    pub already_confirmed: bool,
    pub archive_key_authenticated: bool,
    pub restoration_verified: bool,
}

fn fail(message: &str) -> AppError {
    AppError::Upload(message.into())
}

fn name(path: &Path) -> Result<String> {
    let value = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| fail("Invalid archive filename"))?;
    let stamp = value
        .strip_prefix("backup-")
        .and_then(|s| s.strip_suffix(".tar.gz.enc"))
        .ok_or_else(|| fail("Expected backup-YYYYMMDD-HHMMSS.tar.gz.enc"))?;
    if stamp.len() != 15
        || stamp.as_bytes()[8] != b'-'
        || !stamp
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 8 || b.is_ascii_digit())
    {
        return Err(fail("Invalid archive timestamp filename"));
    }
    Ok(value.into())
}

fn matches(entry: &BackupEntry, filename: &str, size: u64, checksum: &str) -> bool {
    entry.filename == filename && entry.size == size && entry.checksum.as_deref() == Some(checksum)
}

pub async fn execute(
    api: &ApiClient,
    path: &Path,
    expected_sha256: &str,
    key: &[u8; 32],
    driver: &str,
) -> Result<UploadReceipt> {
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(fail("An explicit lowercase SHA-256 checksum is required"));
    }
    if !["mysql", "pgsql", "sqlite"].contains(&driver) {
        return Err(fail("Unsupported database driver"));
    }
    let filename = name(path)?;
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(fail("Archive must be a regular file, not a symlink"));
    }
    // A private encrypted snapshot prevents a changing original from changing
    // the bytes between checksum/authentication and the streamed HTTP request.
    // tempfile creates the file owner-only; copy streams, never loads it in RAM.
    let snapshot = tempfile::NamedTempFile::new()?;
    std::io::copy(&mut std::fs::File::open(path)?, &mut snapshot.as_file())?;
    snapshot.as_file().sync_all()?;
    let size = snapshot.as_file().metadata()?.len();
    let checksum = crypto::checksum(snapshot.path())?;
    if checksum != expected_sha256 {
        return Err(fail("Archive checksum differs; no cloud request made"));
    }
    if !crypto::verify_key(snapshot.path(), key)? {
        return Err(fail("Archive authentication failed; no cloud request made"));
    }
    let mut seen = HashSet::new();
    let mut found = None;
    let mut exhausted = false;
    for page in 1..=10000 {
        let entries = api.list_backups(page, 50).await?;
        if entries.is_empty() {
            exhausted = true;
            break;
        }
        for entry in entries {
            if !seen.insert(entry.id.clone()) {
                return Err(fail(
                    "Cloud pagination repeated an ID; reconcile before retrying",
                ));
            }
            if entry.filename == filename {
                if !matches(&entry, &filename, size, &checksum) || found.is_some() {
                    return Err(fail(
                        "Conflicting or duplicate confirmed archive; reconcile before retrying",
                    ));
                }
                found = Some(entry.id);
            }
        }
    }
    if !exhausted {
        return Err(fail(
            "Cloud pagination ceiling reached; no upload attempted",
        ));
    }
    let already_confirmed = found.is_some();
    let id = match found {
        Some(id) => id,
        None => {
            let requested = api
                .request_upload_url(&filename, size, &checksum, driver)
                .await?;
            upload::upload(snapshot.path(), &requested.upload_url).await?;
            api.confirm_backup(&requested.backup_id, size, &checksum)
                .await?;
            requested.backup_id
        }
    };
    let confirmed = api.get_backup(&id).await?;
    if confirmed.id != id || !matches(&confirmed, &filename, size, &checksum) {
        return Err(fail(
            "Cloud confirmation metadata differs; preserve archive and reconcile",
        ));
    }
    Ok(UploadReceipt {
        id,
        filename,
        size,
        checksum,
        already_confirmed,
        archive_key_authenticated: true,
        restoration_verified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn fixture(dir: &Path) -> (std::path::PathBuf, [u8; 32], String) {
        let plain = dir.join("fixture");
        std::fs::write(&plain, b"synthetic archive contents").unwrap();
        let path = dir.join("backup-20260917-010000.tar.gz.enc");
        let key = crypto::derive_key("synthetic-key");
        crypto::encrypt(&plain, &path, &key).unwrap();
        let hash = crypto::checksum(&path).unwrap();
        (path, key, hash)
    }

    #[tokio::test]
    async fn invalid_archive_or_key_makes_no_cloud_request() {
        let dir = tempfile::tempdir().unwrap();
        let (path, key, hash) = fixture(dir.path());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api = ApiClient::new(
            "synthetic",
            &format!("http://{}", listener.local_addr().unwrap()),
            "test",
        );
        for (checksum, candidate_key) in [
            ("bad".into(), key),
            ("0".repeat(64), key),
            (hash.clone(), [0; 32]),
        ] {
            assert!(
                execute(&api, &path, &checksum, &candidate_key, "mysql")
                    .await
                    .is_err()
            );
        }
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(&path, bytes).unwrap();
        let damaged_hash = crypto::checksum(&path).unwrap();
        assert!(
            execute(&api, &path, &damaged_hash, &key, "mysql")
                .await
                .is_err()
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[tokio::test]
    async fn exact_archive_upload_and_reconciliation_protocol() {
        for mode in [
            "fresh",
            "existing-second-page",
            "conflict",
            "repeated-page",
            "upload-failure",
            "confirmation-mismatch",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let (path, key, hash) = fixture(dir.path());
            let bytes = std::fs::read(&path).unwrap();
            let expected_bytes = bytes.clone();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let api = ApiClient::new("synthetic", &url, "test");
            let entry = serde_json::json!({"id":"bk_fixture", "filename":path.file_name().unwrap().to_str().unwrap(),
                "size":bytes.len(), "checksum":hash, "created_at":"2026-09-17T01:00:00Z"});
            let unrelated = serde_json::json!({"id":"bk_other", "filename":"backup-20260916-010000.tar.gz.enc",
                "size":30, "checksum":"0".repeat(64), "created_at":"2026-09-16T01:00:00Z"});
            let empty = serde_json::json!({"data":[]}).to_string();
            let page = |v: serde_json::Value| serde_json::json!({"data":[v]}).to_string();
            let mut steps: Vec<(String, u16, String)> = Vec::new();
            if mode == "existing-second-page" {
                steps.push((
                    "GET /backups?page=1&per_page=50".into(),
                    200,
                    page(unrelated.clone()),
                ));
                steps.push((
                    "GET /backups?page=2&per_page=50".into(),
                    200,
                    page(entry.clone()),
                ));
                steps.push(("GET /backups?page=3&per_page=50".into(), 200, empty.clone()));
            } else if mode == "repeated-page" {
                steps.push((
                    "GET /backups?page=1&per_page=50".into(),
                    200,
                    page(unrelated.clone()),
                ));
                steps.push((
                    "GET /backups?page=2&per_page=50".into(),
                    200,
                    page(unrelated.clone()),
                ));
            } else if mode == "conflict" {
                let mut conflicting = entry.clone();
                conflicting["checksum"] = serde_json::json!("0".repeat(64));
                steps.push((
                    "GET /backups?page=1&per_page=50".into(),
                    200,
                    page(conflicting),
                ));
            } else {
                steps.push(("GET /backups?page=1&per_page=50".into(), 200, empty));
                steps.push(("POST /upload-url".into(),201,serde_json::json!({"backup_id":"bk_fixture", "upload_url":format!("{url}/object?signature=secret")}).to_string()));
                steps.push((
                    "PUT /object?signature=secret".into(),
                    if mode == "upload-failure" { 503 } else { 200 },
                    String::new(),
                ));
                if mode != "upload-failure" {
                    steps.push(("POST /backups/bk_fixture/confirm".into(), 200, "{}".into()));
                }
            }
            if ["fresh", "existing-second-page", "confirmation-mismatch"].contains(&mode) {
                let mut confirmed = entry.clone();
                if mode == "confirmation-mismatch" {
                    confirmed["size"] = serde_json::json!(1);
                }
                steps.push(("GET /backups/bk_fixture".into(), 200, confirmed.to_string()));
            }
            let server = std::thread::spawn(move || {
                for (request, status, response) in steps {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                    let mut socket = loop {
                        match listener.accept() {
                            Ok((socket, _)) => break socket,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(
                                    std::time::Instant::now() < deadline,
                                    "missing request: {request}"
                                );
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(e) => panic!("{e}"),
                        }
                    };
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    let mut reader = BufReader::new(socket.try_clone().unwrap());
                    let mut first = String::new();
                    reader.read_line(&mut first).unwrap();
                    assert_eq!(first.trim_end(), format!("{request} HTTP/1.1"));
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        assert!(reader.read_line(&mut line).unwrap() > 0);
                        if line == "\r\n" {
                            break;
                        }
                        if let Some(value) =
                            line.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            length = value.trim().parse().unwrap();
                        }
                    }
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body).unwrap();
                    if request.starts_with("PUT ") {
                        assert_eq!(body, expected_bytes);
                    }
                    if request == "POST /upload-url" || request.ends_with("/confirm") {
                        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(payload["checksum"], entry["checksum"]);
                        assert_eq!(payload["size"], entry["size"]);
                    }
                    write!(socket,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
                }
            });
            let result = execute(&api, &path, &hash, &key, "mysql").await;
            server.join().unwrap();
            if ["fresh", "existing-second-page"].contains(&mode) {
                let receipt = result.unwrap();
                assert_eq!(receipt.already_confirmed, mode == "existing-second-page");
                assert!(receipt.archive_key_authenticated);
                assert!(!receipt.restoration_verified);
                assert_eq!(receipt.checksum, hash);
            } else {
                assert!(result.is_err(), "{mode}");
            }
            assert_eq!(
                std::fs::read(&path).unwrap(),
                bytes,
                "original must remain untouched"
            );
        }
    }
}
