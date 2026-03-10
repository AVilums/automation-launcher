use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::LauncherError;

/// Extract a zip archive to the given destination directory.
pub fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<Vec<PathBuf>, LauncherError> {
    std::fs::create_dir_all(dest_dir)?;

    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::Download(format!("Failed to open zip: {}", e)))?;

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| LauncherError::Download(format!("Zip entry error: {}", e)))?;

        let out_path = dest_dir.join(
            entry
                .enclosed_name()
                .ok_or_else(|| LauncherError::Download("Invalid zip entry name".into()))?,
        );

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            extracted.push(out_path);
        }
    }

    info!(
        "Extracted {} files from {}",
        extracted.len(),
        archive_path.display()
    );
    Ok(extracted)
}

/// Detect whether a file is a zip archive by checking the magic bytes.
pub fn is_zip(path: &Path) -> bool {
    std::fs::read(path)
        .map(|bytes| bytes.len() >= 4 && bytes[0..4] == [0x50, 0x4B, 0x03, 0x04])
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_zip() {
        let tmp = TempDir::new().unwrap();
        let zip_path = tmp.path().join("test.zip");

        // Create a minimal zip with one file
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("hello.txt", options).unwrap();
        writer.write_all(b"Hello, world!").unwrap();
        writer.finish().unwrap();

        let dest = tmp.path().join("out");
        let files = extract_zip(&zip_path, &dest).unwrap();
        assert_eq!(files.len(), 1);
        assert!(dest.join("hello.txt").exists());

        let content = std::fs::read_to_string(dest.join("hello.txt")).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[test]
    fn test_is_zip() {
        let tmp = TempDir::new().unwrap();

        // Create a real zip
        let zip_path = tmp.path().join("test.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("a.txt", options).unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();

        assert!(is_zip(&zip_path));

        // Non-zip file
        let txt_path = tmp.path().join("plain.txt");
        std::fs::write(&txt_path, b"not a zip").unwrap();
        assert!(!is_zip(&txt_path));
    }

    #[test]
    fn test_extract_zip_nested() {
        let tmp = TempDir::new().unwrap();
        let zip_path = tmp.path().join("nested.zip");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        writer.add_directory("subdir/", options).unwrap();
        writer.start_file("subdir/inner.txt", options).unwrap();
        writer.write_all(b"nested content").unwrap();
        writer.finish().unwrap();

        let dest = tmp.path().join("out");
        let files = extract_zip(&zip_path, &dest).unwrap();
        assert_eq!(files.len(), 1);
        assert!(dest.join("subdir").join("inner.txt").exists());
    }
}
