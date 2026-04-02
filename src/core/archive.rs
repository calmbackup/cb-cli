use std::path::{Path, PathBuf};
use crate::core::types::{AppError, Result};

/// Create a tar.gz archive containing the database dump and optional directories.
/// The dump file is placed at the archive root (renamed to its basename).
/// Each directory is added as a top-level folder (by basename).
pub fn create(dump_path: &Path, directories: &[String], output_path: &Path) -> Result<()> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs::File;

    let file = File::create(output_path)?;
    let encoder = GzEncoder::new(file, Compression::Default);
    let mut builder = tar::Builder::new(encoder);

    // Add the dump file at the archive root using its basename
    let dump_name = dump_path
        .file_name()
        .ok_or_else(|| AppError::Archive("dump path has no filename".to_string()))?;
    builder
        .append_path_with_name(dump_path, dump_name)
        .map_err(|e| AppError::Archive(format!("failed to add dump file: {}", e)))?;

    // Add each directory recursively as a top-level folder by basename
    for dir in directories {
        let dir_path = Path::new(dir);
        let dir_name = dir_path
            .file_name()
            .ok_or_else(|| AppError::Archive(format!("directory has no basename: {}", dir)))?;
        builder
            .append_dir_all(dir_name, dir_path)
            .map_err(|e| AppError::Archive(format!("failed to add directory {}: {}", dir, e)))?;
    }

    // Finish writing the archive
    let encoder = builder
        .into_inner()
        .map_err(|e| AppError::Archive(format!("failed to finish archive: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| AppError::Archive(format!("failed to finish gzip: {}", e)))?;

    Ok(())
}

/// Extract a tar.gz archive to the given output directory.
/// Returns the list of extracted file paths.
/// Sanitizes paths to prevent directory traversal attacks.
pub fn extract(archive_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    use flate2::read::GzDecoder;
    use std::fs::File;

    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);

    let mut extracted = Vec::new();

    for entry in archive
        .entries()
        .map_err(|e| AppError::Archive(format!("failed to read archive entries: {}", e)))?
    {
        let mut entry =
            entry.map_err(|e| AppError::Archive(format!("failed to read entry: {}", e)))?;

        let path = entry
            .path()
            .map_err(|e| AppError::Archive(format!("failed to read entry path: {}", e)))?
            .to_path_buf();

        // Reject any path containing ".." components
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                return Err(AppError::Archive(format!(
                    "path traversal detected in archive entry: {}",
                    path.display()
                )));
            }
        }

        let full_path = output_dir.join(&path);
        entry
            .unpack(&full_path)
            .map_err(|e| AppError::Archive(format!("failed to unpack {}: {}", path.display(), e)))?;

        extracted.push(full_path);
    }

    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a unique temp directory for test isolation.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cb_archive_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_and_extract_roundtrip() {
        let dir = temp_dir("roundtrip");
        let dump = dir.join("database.sql");
        fs::write(&dump, "CREATE TABLE test; INSERT INTO test VALUES (1);").unwrap();

        let archive_path = dir.join("backup.tar.gz");
        create(&dump, &[], &archive_path).unwrap();
        assert!(archive_path.exists());

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        let extracted = extract(&archive_path, &extract_dir).unwrap();

        assert!(!extracted.is_empty());
        let content = fs::read_to_string(extract_dir.join("database.sql")).unwrap();
        assert_eq!(content, "CREATE TABLE test; INSERT INTO test VALUES (1);");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn create_with_directories() {
        let dir = temp_dir("with_dirs");
        let dump = dir.join("dump.sql");
        fs::write(&dump, "DUMP DATA").unwrap();

        let subdir = dir.join("migrations");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("001.sql"), "ALTER TABLE foo;").unwrap();
        fs::write(subdir.join("002.sql"), "ALTER TABLE bar;").unwrap();

        let archive_path = dir.join("backup.tar.gz");
        create(
            &dump,
            &[subdir.to_string_lossy().to_string()],
            &archive_path,
        )
        .unwrap();

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        extract(&archive_path, &extract_dir).unwrap();

        assert_eq!(fs::read_to_string(extract_dir.join("dump.sql")).unwrap(), "DUMP DATA");
        assert_eq!(
            fs::read_to_string(extract_dir.join("migrations").join("001.sql")).unwrap(),
            "ALTER TABLE foo;"
        );
        assert_eq!(
            fs::read_to_string(extract_dir.join("migrations").join("002.sql")).unwrap(),
            "ALTER TABLE bar;"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn extract_rejects_path_traversal() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let dir = temp_dir("traversal");
        let archive_path = dir.join("evil.tar.gz");

        // Build a tar.gz with a ../evil.txt entry by writing raw tar bytes.
        // The tar crate rejects ".." in set_path, so we craft the header manually.
        let mut tar_bytes: Vec<u8> = Vec::new();
        {
            let mut header = tar::Header::new_gnu();
            // Set a safe placeholder path first, then overwrite the name bytes
            header.set_path("safe.txt").unwrap();
            header.set_size(17); // len of "malicious content"
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);

            // Overwrite the name field (first 100 bytes) with "../evil.txt"
            let evil_path = b"../evil.txt";
            let raw = header.as_mut_bytes();
            raw[..100].fill(0);
            raw[..evil_path.len()].copy_from_slice(evil_path);

            // Recompute checksum
            header.set_cksum();

            tar_bytes.extend_from_slice(header.as_bytes());
        }
        // File data: "malicious content" (17 bytes), padded to 512
        let data = b"malicious content";
        tar_bytes.extend_from_slice(data);
        tar_bytes.resize(tar_bytes.len() + (512 - data.len() % 512) % 512, 0);
        // Two zero blocks to end the tar
        tar_bytes.extend_from_slice(&[0u8; 1024]);

        // Gzip-compress the crafted tar
        let file = fs::File::create(&archive_path).unwrap();
        let mut encoder = GzEncoder::new(file, Compression::Default);
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap();

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();

        let result = extract(&archive_path, &extract_dir);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Archive(msg) => assert!(
                msg.contains("path traversal"),
                "expected path traversal error, got: {}",
                msg
            ),
            other => panic!("expected AppError::Archive, got: {:?}", other),
        }

        // Verify evil.txt was NOT created outside the extract dir
        assert!(!dir.join("evil.txt").exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dump_file_at_archive_root() {
        let dir = temp_dir("root_placement");

        // Create dump in a nested directory to test that create() strips parents
        let nested = dir.join("deep").join("nested");
        fs::create_dir_all(&nested).unwrap();
        let dump = nested.join("my_dump.sql");
        fs::write(&dump, "ROOT CHECK").unwrap();

        let archive_path = dir.join("backup.tar.gz");
        create(&dump, &[], &archive_path).unwrap();

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        let extracted = extract(&archive_path, &extract_dir).unwrap();

        // The dump should be at the root, not nested under deep/nested/
        assert!(extracted.contains(&extract_dir.join("my_dump.sql")));
        assert_eq!(
            fs::read_to_string(extract_dir.join("my_dump.sql")).unwrap(),
            "ROOT CHECK"
        );
        assert!(!extract_dir.join("deep").exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn preserves_file_content_bytes() {
        let dir = temp_dir("byte_content");
        let dump = dir.join("binary.dump");
        let content: Vec<u8> = (0..=255).collect();
        fs::write(&dump, &content).unwrap();

        let archive_path = dir.join("backup.tar.gz");
        create(&dump, &[], &archive_path).unwrap();

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        extract(&archive_path, &extract_dir).unwrap();

        let restored = fs::read(extract_dir.join("binary.dump")).unwrap();
        assert_eq!(restored, content);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_directories_list() {
        let dir = temp_dir("empty_dirs");
        let dump = dir.join("dump.sql");
        fs::write(&dump, "SIMPLE DUMP").unwrap();

        let archive_path = dir.join("backup.tar.gz");
        create(&dump, &[], &archive_path).unwrap();
        assert!(archive_path.exists());

        let extract_dir = dir.join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        let extracted = extract(&archive_path, &extract_dir).unwrap();
        assert_eq!(extracted.len(), 1);
        assert_eq!(
            fs::read_to_string(extract_dir.join("dump.sql")).unwrap(),
            "SIMPLE DUMP"
        );

        fs::remove_dir_all(&dir).unwrap();
    }
}
