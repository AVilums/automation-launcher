use tracing::{info, warn};

use crate::config::{BootstrapConfig, KillSwitchConfig};
use crate::error::BootstrapError;

pub async fn check_killswitch(
    config: &BootstrapConfig,
) -> Result<Option<KillSwitchConfig>, BootstrapError> {
    let url = match &config.killswitch_url {
        Some(url) => url,
        None => {
            info!("No kill-switch URL configured, skipping check");
            return Ok(None);
        }
    };

    info!("Checking kill-switch at {}", url);

    let client = reqwest::Client::new();
    let response = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Failed to check kill-switch (proceeding normally): {}", e);
            return Ok(None);
        }
    };

    let ks: KillSwitchConfig = response.json().await?;
    Ok(Some(ks))
}

pub fn enforce_killswitch(
    config: &BootstrapConfig,
    ks: &KillSwitchConfig,
) -> Result<(), BootstrapError> {
    if let Some(msg) = &ks.message {
        eprintln!("NOTICE: {}", msg);
    }

    let launcher_path = &config.launcher_path;
    if launcher_path.exists() {
        info!("Removing launcher binary at {:?}", launcher_path);
        std::fs::remove_file(launcher_path)?;
        info!("Launcher binary removed");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_enforce_killswitch_removes_launcher() {
        let tmp = TempDir::new().unwrap();
        let launcher_path = tmp.path().join("launcher.exe");
        std::fs::write(&launcher_path, b"fake binary").unwrap();

        let config = BootstrapConfig {
            launcher_path: launcher_path.clone(),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: tmp.path().to_path_buf(),
        };

        let ks = KillSwitchConfig {
            enabled: true,
            message: Some("System disabled".into()),
        };

        enforce_killswitch(&config, &ks).unwrap();
        assert!(!launcher_path.exists());
    }

    #[test]
    fn test_enforce_killswitch_no_file() {
        let tmp = TempDir::new().unwrap();
        let config = BootstrapConfig {
            launcher_path: tmp.path().join("nonexistent.exe"),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: tmp.path().to_path_buf(),
        };

        let ks = KillSwitchConfig {
            enabled: true,
            message: None,
        };

        enforce_killswitch(&config, &ks).unwrap();
    }

    #[tokio::test]
    async fn test_check_killswitch_no_url() {
        let config = BootstrapConfig {
            launcher_path: PathBuf::from("launcher.exe"),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: PathBuf::from("/tmp"),
        };

        let result = check_killswitch(&config).await.unwrap();
        assert!(result.is_none());
    }
}
