use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::error::LauncherError;

pub struct DownloadManager {
    client: reqwest::Client,
    downloads_dir: PathBuf,
    max_retries: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
}

impl DownloadManager {
    pub fn new(downloads_dir: PathBuf) -> Self {
        Self {
            client: reqwest::Client::new(),
            downloads_dir,
            max_retries: 3,
        }
    }

    #[allow(dead_code)]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub async fn download(
        &self,
        url: &str,
        expected_sha256: &str,
    ) -> Result<PathBuf, LauncherError> {
        std::fs::create_dir_all(&self.downloads_dir)?;

        let filename = url
            .rsplit('/')
            .next()
            .unwrap_or("download");
        let dest_path = self.downloads_dir.join(filename);

        let mut last_error = None;
        for attempt in 1..=self.max_retries {
            info!("Download attempt {}/{} for {}", attempt, self.max_retries, url);

            match self.download_with_resume(url, &dest_path).await {
                Ok(()) => {
                    info!("Download complete, verifying checksum");
                    let actual_hash = self.compute_file_sha256(&dest_path).await?;
                    if actual_hash != expected_sha256 {
                        let _ = std::fs::remove_file(&dest_path);
                        return Err(LauncherError::ChecksumMismatch {
                            expected: expected_sha256.to_string(),
                            actual: actual_hash,
                        });
                    }
                    info!("Checksum verified successfully");
                    return Ok(dest_path);
                }
                Err(e) => {
                    warn!("Download attempt {} failed: {}", attempt, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            LauncherError::Download("All download attempts failed".into())
        }))
    }

    async fn download_with_resume(
        &self,
        url: &str,
        dest_path: &Path,
    ) -> Result<(), LauncherError> {
        let existing_size = if dest_path.exists() {
            std::fs::metadata(dest_path)?.len()
        } else {
            0
        };

        let mut request = self.client.get(url);
        if existing_size > 0 {
            info!("Resuming download from byte {}", existing_size);
            request = request.header("Range", format!("bytes={}-", existing_size));
        }

        let response = request.send().await?;
        let status = response.status();

        let append = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let mut file = if append {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(dest_path)
                .await?
        } else {
            tokio::fs::File::create(dest_path).await?
        };

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(LauncherError::Network)?;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        Ok(())
    }

    async fn compute_file_sha256(&self, path: &Path) -> Result<String, LauncherError> {
        let data = tokio::fs::read(path).await?;
        Ok(compute_sha256(&data))
    }
}

pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[allow(dead_code)]
pub fn verify_checksum(data: &[u8], expected: &str) -> Result<(), LauncherError> {
    let actual = compute_sha256(data);
    if actual != expected {
        return Err(LauncherError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_sha256_empty() {
        let hash = compute_sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_verify_checksum_success() {
        let data = b"hello world";
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_checksum(data, expected).is_ok());
    }

    #[test]
    fn test_verify_checksum_failure() {
        let data = b"hello world";
        let result = verify_checksum(data, "wrong_hash");
        assert!(result.is_err());
        match result.unwrap_err() {
            LauncherError::ChecksumMismatch { expected, actual } => {
                assert_eq!(expected, "wrong_hash");
                assert_eq!(
                    actual,
                    "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
                );
            }
            _ => panic!("Expected ChecksumMismatch error"),
        }
    }

    #[test]
    fn test_download_manager_creation() {
        let dm = DownloadManager::new(PathBuf::from("/tmp/downloads"));
        assert_eq!(dm.max_retries, 3);

        let dm = dm.with_retries(5);
        assert_eq!(dm.max_retries, 5);
    }
}
