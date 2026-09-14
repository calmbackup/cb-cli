//! Opt-in real file/HTTP test. Run the compiled test executable in a memory-
//! limited container; compiling under the same tiny limit is not the test.
use super::{crypto, dumper::mysql, upload};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

fn memory_snapshot(stage: &str) {
    println!("Memory snapshot: {stage}");
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status
            .lines()
            .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        {
            println!("{line}");
        }
    }
    if let Ok(stats) = std::fs::read_to_string("/sys/fs/cgroup/memory.stat") {
        for line in stats.lines().filter(|line| {
            [
                "anon ",
                "file ",
                "file_dirty ",
                "file_writeback ",
                "shmem ",
                "kernel ",
            ]
            .iter()
            .any(|key| line.starts_with(key))
        }) {
            println!("cgroup {line}");
        }
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "multi-GiB disk/network test; see MEMORY-EFFICIENT-BACKUPS.md"]
async fn large_file_pipeline_under_memory_limit() {
    // Optional low-frequency observer for failures inside an individual stage.
    // It reads small proc/cgroup metadata only, never payload contents.
    let (finish, observer) = if std::env::var_os("CB_MEMORY_DIAGNOSTICS").is_some() {
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            while matches!(
                rx.recv_timeout(std::time::Duration::from_millis(100)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ) {
                memory_snapshot("periodic observer");
            }
        });
        (Some(tx), Some(thread))
    } else {
        (None, None)
    };
    let gib: u64 = std::env::var("CB_MEMORY_TEST_GIB")
        .unwrap_or("2".into())
        .parse()
        .unwrap();
    assert!((1..=32).contains(&gib));
    let bytes = gib * 1024 * 1024 * 1024 + 37;
    let dir = match std::env::var("CB_MEMORY_TEST_DIR") {
        Ok(path) => tempfile::Builder::new()
            .prefix("cb-memory-test-")
            .tempdir_in(path)
            .unwrap(),
        Err(_) => tempfile::tempdir().unwrap(),
    };
    let input = dir.path().join("input");
    let encrypted = dir.path().join("encrypted");
    let downloaded = dir.path().join("downloaded");
    let restored = dir.path().join("restored");
    let mut file = File::create(&input).unwrap();
    let mut buffer = [0u8; 65536];
    for (i, b) in buffer.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let mut remaining = bytes;
    println!("Generating {bytes} bytes of synthetic data (no production access)");
    memory_snapshot("before fixture generation");
    let mut next_snapshot = 64 * 1024 * 1024;
    while remaining > 0 {
        buffer[..8].copy_from_slice(&remaining.to_le_bytes());
        let n = remaining.min(buffer.len() as u64) as usize;
        file.write_all(&buffer[..n]).unwrap();
        remaining -= n as u64;
        if bytes - remaining >= next_snapshot {
            memory_snapshot(&format!(
                "generated {} MiB",
                (bytes - remaining) / 1024 / 1024
            ));
            next_snapshot += 64 * 1024 * 1024;
        }
    }
    file.sync_all().unwrap();
    drop(file);
    let key = crypto::derive_key("synthetic-memory-test-key-not-for-backups");
    println!("Encrypting complete file");
    memory_snapshot("before encryption");
    crypto::encrypt(&input, &encrypted, &key).unwrap();
    let encrypted_bytes = std::fs::metadata(&encrypted).unwrap().len();
    assert_eq!(encrypted_bytes, bytes + 30);
    println!("Checksumming and authenticating encrypted file");
    memory_snapshot("before checksum and authentication");
    let checksum = crypto::checksum(&encrypted).unwrap();
    assert!(crypto::verify_key(&encrypted, &key).unwrap());

    // The loopback server also uses bounded buffers and never stores the PUT in
    // RAM. A SHA-256 verifies every uploaded byte, not only the byte count.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/backup", listener.local_addr().unwrap());
    let server_file = encrypted.clone();
    let server = std::thread::spawn(move || {
        let mut uploaded_hash = String::new();
        for expected_method in ["PUT", "GET"] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(300)))
                .unwrap();
            socket
                .set_write_timeout(Some(std::time::Duration::from_secs(300)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with(expected_method));
            let mut length = None;
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = Some(value.trim().parse::<u64>().unwrap());
                }
            }
            if expected_method == "PUT" {
                assert_eq!(length, Some(encrypted_bytes));
                let mut left = encrypted_bytes;
                let mut block = [0; 65536];
                let mut hash = Sha256::new();
                while left > 0 {
                    let n = left.min(block.len() as u64) as usize;
                    reader.read_exact(&mut block[..n]).unwrap();
                    hash.update(&block[..n]);
                    left -= n as u64;
                }
                uploaded_hash = hex::encode(hash.finalize());
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            } else {
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {encrypted_bytes}\r\nConnection: close\r\n\r\n").unwrap();
                assert_eq!(
                    std::io::copy(&mut File::open(&server_file).unwrap(), &mut socket).unwrap(),
                    encrypted_bytes
                );
            }
        }
        uploaded_hash
    });
    println!("Uploading over loopback HTTP");
    memory_snapshot("before upload");
    upload::upload(&encrypted, &url).await.unwrap();
    println!("Downloading over loopback HTTP");
    memory_snapshot("before download");
    upload::download(&url, &downloaded).await.unwrap();
    assert_eq!(server.join().unwrap(), checksum);
    assert_eq!(crypto::checksum(&downloaded).unwrap(), checksum);
    println!("Decrypting and comparing every plaintext byte");
    memory_snapshot("before decryption");
    crypto::decrypt(&downloaded, &restored, &key).unwrap();
    assert_eq!(std::fs::metadata(&restored).unwrap().len(), bytes);
    let mut original = File::open(&input).unwrap();
    let mut recovered = File::open(&restored).unwrap();
    let mut compare = [0; 65536];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(buffer.len() as u64) as usize;
        original.read_exact(&mut buffer[..n]).unwrap();
        recovered.read_exact(&mut compare[..n]).unwrap();
        assert_eq!(buffer[..n], compare[..n]);
        left -= n as u64;
    }
    if let Ok(path) = std::env::var("CB_MYSQL_DUMP") {
        assert!(mysql::verify_dump(std::path::Path::new(&path)).unwrap());
        println!("Read-only supplied MySQL dump trailer verification PASS");
    }
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines().filter(|line| line.starts_with("VmHWM:")) {
            println!("{line}");
        }
    }
    println!(
        "PASS: {bytes} plaintext bytes; {encrypted_bytes} encrypted bytes; full HTTP/checksum/authentication/byte equality"
    );
    if let Some(tx) = finish {
        tx.send(()).unwrap();
    }
    if let Some(thread) = observer {
        thread.join().unwrap();
    }
    // TempDir cleans only this test's exclusively generated files.
}
