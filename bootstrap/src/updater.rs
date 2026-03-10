use sha2::{Digest, Sha256};
use tracing::info;

use crate::config::{BootstrapConfig, UpdateMetadata};
use crate::error::BootstrapError;

pub async fn check_for_update(config: &BootstrapConfig) -> Result<bool, BootstrapError> {
    let remote = fetch_update_metadata(config).await?;
    let local_version = read_local_version(config);

    match local_version {
        Some(v) if v == remote.version => {
            info!("Launcher is up to date (version {})", v);
            Ok(false)
        }
        Some(v) => {
            info!(
                "Update available: local={}, remote={}",
                v, remote.version
            );
            Ok(true)
        }
        None => {
            info!("No local version found, update needed");
            Ok(true)
        }
    }
}

pub async fn perform_update(config: &BootstrapConfig) -> Result<(), BootstrapError> {
    let metadata = fetch_update_metadata(config).await?;

    info!("Downloading launcher from {}", metadata.download_url);
    let client = reqwest::Client::new();
    let bytes = client
        .get(&metadata.download_url)
        .send()
        .await?
        .bytes()
        .await?;

    let hash = compute_sha256(&bytes);
    if hash != metadata.sha256 {
        return Err(BootstrapError::ChecksumMismatch {
            expected: metadata.sha256,
            actual: hash,
        });
    }

    info!("Checksum verified, writing launcher binary");
    if let Some(parent) = config.launcher_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config.launcher_path, &bytes)?;

    save_local_version(config, &metadata.version)?;
    info!("Launcher updated to version {}", metadata.version);
    Ok(())
}

pub fn launch_launcher(config: &BootstrapConfig) -> Result<(), BootstrapError> {
    if !config.launcher_path.exists() {
        return Err(BootstrapError::LauncherNotFound(
            config.launcher_path.display().to_string(),
        ));
    }

    info!("Starting launcher at {:?}", config.launcher_path);
    std::process::Command::new(&config.launcher_path).spawn()?;
    Ok(())
}

async fn fetch_update_metadata(
    config: &BootstrapConfig,
) -> Result<UpdateMetadata, BootstrapError> {
    let client = reqwest::Client::new();
    let response = client.get(&config.update_url).send().await?;
    let metadata: UpdateMetadata = response.json().await?;
    Ok(metadata)
}

fn read_local_version(config: &BootstrapConfig) -> Option<String> {
    let path = config.version_file();
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value.get("version")?.as_str().map(|s| s.to_string())
}

fn save_local_version(config: &BootstrapConfig, version: &str) -> Result<(), BootstrapError> {
    let path = config.version_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let meta = serde_json::json!({ "version": version });
    std::fs::write(path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_save_and_read_local_version() {
        let tmp = TempDir::new().unwrap();
        let config = BootstrapConfig {
            launcher_path: tmp.path().join("launcher.exe"),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: tmp.path().to_path_buf(),
        };

        assert!(read_local_version(&config).is_none());

        save_local_version(&config, "1.2.3").unwrap();
        assert_eq!(read_local_version(&config), Some("1.2.3".into()));
    }

    #[test]
    fn test_launch_launcher_not_found() {
        let config = BootstrapConfig {
            launcher_path: std::path::PathBuf::from("/nonexistent/launcher.exe"),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: std::path::PathBuf::from("/tmp"),
        };

        let result = launch_launcher(&config);
        assert!(result.is_err());
    }
}
