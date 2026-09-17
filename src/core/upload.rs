use std::path::Path;

use crate::core::staging;
use crate::core::types::{AppError, Result};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

/// Report categories only: error source text may contain signed URLs or secrets.
fn transport_diagnostic(error: &reqwest::Error) -> String {
    let mut io_kind = None;
    let mut source = std::error::Error::source(error);
    for _ in 0..16 {
        let Some(current) = source else { break };
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            io_kind = Some(io.kind());
        }
        source = current.source();
    }
    format!(
        "timeout={}, connect={}, request={}, body={}, decode={}, io_kind={:?}",
        error.is_timeout(),
        error.is_connect(),
        error.is_request(),
        error.is_body(),
        error.is_decode(),
        io_kind
    )
}

/// Upload an encrypted backup file to a presigned URL via HTTP PUT.
pub async fn upload(file_path: &Path, presigned_url: &str) -> Result<()> {
    let file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| AppError::Upload(format!("failed to read file: {}", e)))?;

    let size = file.metadata().await?.len();
    let body = reqwest::Body::wrap_stream(ReaderStream::with_capacity(file, 64 * 1024));

    // A streamed request cannot be replayed safely across redirects.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Upload(e.without_url().to_string()))?;
    let response = client
        .put(presigned_url)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", size)
        .body(body)
        .send()
        .await
        .map_err(|e| {
            AppError::Upload(format!(
                "upload request failed: {}",
                transport_diagnostic(&e)
            ))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        // Do not buffer an arbitrary error body or log a signed URL.
        return Err(AppError::Upload(format!(
            "upload failed with status {}",
            status
        )));
    }

    Ok(())
}

/// Download a backup file from a presigned URL via HTTP GET.
/// Atomically publishes a completed download; failure/cancellation removes only
/// its private temporary file, never an existing cache entry.
pub async fn download(url: &str, output_path: &Path) -> Result<()> {
    let client = reqwest::Client::new();
    let mut response =
        client.get(url).send().await.map_err(|e| {
            AppError::Download(format!("download request failed: {}", e.without_url()))
        })?;

    if !response.status().is_success() {
        return Err(AppError::Download(format!(
            "download failed with status {}",
            response.status()
        )));
    }

    let staged = staging::file(output_path)?;
    let mut file = tokio::fs::File::from_std(staged.as_file().try_clone()?);
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        AppError::Download(format!("failed to read response body: {}", e.without_url()))
    })? {
        file.write_all(&chunk).await?;
    }

    file.flush()
        .await
        .map_err(|e| AppError::Download(format!("failed to flush file: {}", e)))?;

    file.sync_all().await?;
    drop(file);
    staging::publish(staged, output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn transport_diagnostic_reports_timeout_without_signed_url() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/secret-path?signature=secret-value",
            listener.local_addr().unwrap()
        );
        // Keep the listener open but send no response: deterministic local timeout.
        let error = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(50))
            .build()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap_err();
        let detail = transport_diagnostic(&error);
        assert!(detail.contains("timeout=true"));
        assert!(!detail.contains("secret"));
        assert!(!detail.contains("127.0.0.1"));
        drop(listener);
    }

    #[tokio::test]
    async fn upload_reports_transport_categories_without_signed_url() {
        let (url, thread) = server(b"");
        let file = tempfile::NamedTempFile::new().unwrap();
        let error = upload(file.path(), &url).await.unwrap_err().to_string();
        assert!(error.contains("upload request failed: timeout="));
        assert!(error.contains("io_kind="));
        assert!(!error.contains("signature"));
        assert!(!error.contains("127.0.0.1"));
        thread.join().unwrap();
    }

    fn server(reply: &'static [u8]) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            socket.write_all(reply).unwrap();
        });
        (format!("http://{address}/backup?signature=secret"), thread)
    }

    #[tokio::test]
    async fn downloads_chunked_response_and_publishes_complete_file() {
        let (url, thread) = server(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n4\r\ndefg\r\n0\r\n\r\n");
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("backup");
        download(&url, &output).await.unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"abcdefg");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn download_failure_preserves_existing_file_and_removes_partial() {
        for reply in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort".as_slice(),
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ] {
            let (url, thread) = server(reply);
            let dir = tempfile::tempdir().unwrap();
            let output = dir.path().join("backup");
            std::fs::write(&output, b"previous cache").unwrap();
            let err = download(&url, &output).await.unwrap_err();
            assert!(!err.to_string().contains("signature"));
            assert_eq!(std::fs::read(output).unwrap(), b"previous cache");
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
            thread.join().unwrap();
        }
    }

    #[tokio::test]
    async fn uploads_exact_bytes_with_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/backup", listener.local_addr().unwrap());
        let expected: Vec<u8> = (0..200_003).map(|i| (i % 251) as u8).collect();
        let data = expected.clone();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line.to_lowercase());
            }
            assert!(headers.starts_with("put /backup http/1.1"));
            assert!(headers.contains(&format!("content-length: {}", data.len())));
            assert!(!headers.contains("transfer-encoding"));
            let mut body = vec![0; data.len()];
            reader.read_exact(&mut body).unwrap();
            assert_eq!(body, data);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
        });
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), expected).unwrap();
        upload(file.path(), &url).await.unwrap();
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn upload_rejects_error_and_redirect_responses() {
        for reply in [
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
            b"HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:9/unused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ] {
            let (url, thread) = server(reply);
            let file = tempfile::NamedTempFile::new().unwrap();
            let err = upload(file.path(), &url).await.unwrap_err();
            assert!(!err.to_string().contains("signature"));
            thread.join().unwrap();
        }
    }

    #[tokio::test]
    async fn cancelled_download_removes_staging_and_preserves_cache() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/backup", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
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
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\npartial").unwrap();
            let _ = wait.recv_timeout(std::time::Duration::from_secs(10));
        });
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("backup");
        std::fs::write(&destination, b"original").unwrap();
        let output = destination.clone();
        let task = tokio::spawn(async move { download(&url, &output).await });
        let mut staging_seen = false;
        for _ in 0..200 {
            if std::fs::read_dir(dir.path()).unwrap().count() == 2 {
                staging_seen = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        release.send(()).unwrap();
        server.join().unwrap();
        assert!(staging_seen, "download never reached its staging phase");
        assert_eq!(std::fs::read(destination).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
