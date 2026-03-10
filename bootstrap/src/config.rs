use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::BootstrapError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub launcher_path: PathBuf,
    pub update_url: String,
    pub killswitch_url: Option<String>,
    pub base_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadata {
    pub version: String,
    pub download_url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchConfig {
    pub enabled: bool,
    pub message: Option<String>,
}

impl BootstrapConfig {
    pub fn load() -> Result<Self, BootstrapError> {
        let base_dir = Self::default_base_dir()?;
        let config_path = base_dir.join("config").join("bootstrap.json");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: BootstrapConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = Self::default_config(base_dir);
            config.save()?;
            Ok(config)
        }
    }

    #[allow(dead_code)]
    pub fn load_from(path: &Path) -> Result<Self, BootstrapError> {
        let content = std::fs::read_to_string(path)?;
        let config: BootstrapConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), BootstrapError> {
        let config_dir = self.base_dir.join("config");
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("bootstrap.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn default_base_dir() -> Result<PathBuf, BootstrapError> {
        dirs::data_dir()
            .map(|p| p.join("AutomationLauncher"))
            .ok_or_else(|| BootstrapError::Config("Cannot determine app data directory".into()))
    }

    fn default_config(base_dir: PathBuf) -> Self {
        Self {
            launcher_path: base_dir.join("launcher.exe"),
            update_url: String::from("https://localhost/update/metadata.json"),
            killswitch_url: None,
            base_dir,
        }
    }

    pub fn version_file(&self) -> PathBuf {
        self.base_dir.join("metadata").join("launcher_version.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let base_dir = tmp.path().to_path_buf();

        let config = BootstrapConfig {
            launcher_path: base_dir.join("launcher.exe"),
            update_url: "https://example.com/update".into(),
            killswitch_url: Some("https://example.com/killswitch".into()),
            base_dir: base_dir.clone(),
        };

        config.save().unwrap();

        let config_path = base_dir.join("config").join("bootstrap.json");
        let loaded = BootstrapConfig::load_from(&config_path).unwrap();

        assert_eq!(loaded.update_url, "https://example.com/update");
        assert_eq!(
            loaded.killswitch_url,
            Some("https://example.com/killswitch".into())
        );
    }

    #[test]
    fn test_version_file_path() {
        let config = BootstrapConfig {
            launcher_path: PathBuf::from("launcher.exe"),
            update_url: String::new(),
            killswitch_url: None,
            base_dir: PathBuf::from("/app"),
        };

        let vf = config.version_file();
        assert!(vf.ends_with("launcher_version.json"));
    }
}
