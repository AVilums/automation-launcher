use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::LauncherError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    pub base_dir: PathBuf,
    pub provider: ProviderConfig,
    pub auth: AuthConfig,
    pub cache: CacheConfig,
    pub telemetry: TelemetryConfig,
    pub offline_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderConfig {
    #[serde(rename = "github")]
    GitHub { owner: String, repo: String },
    #[serde(rename = "http")]
    Http { base_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthConfig {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "api_key")]
    ApiKey { key: String },
    #[serde(rename = "jwt")]
    Jwt { token_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub remote_endpoint: Option<String>,
}

impl LauncherConfig {
    pub fn load() -> Result<Self, LauncherError> {
        let base_dir = Self::default_base_dir()?;
        let config_path = base_dir.join("config").join("launcher.json");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: LauncherConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = Self::default_config(base_dir);
            config.save()?;
            Ok(config)
        }
    }

    pub fn load_from(path: &Path) -> Result<Self, LauncherError> {
        let content = std::fs::read_to_string(path)?;
        let config: LauncherConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), LauncherError> {
        let config_dir = self.base_dir.join("config");
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("launcher.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.base_dir.join("downloads")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.base_dir.join("metadata")
    }

    fn default_base_dir() -> Result<PathBuf, LauncherError> {
        dirs::data_dir()
            .map(|p| p.join("AutomationLauncher"))
            .ok_or_else(|| LauncherError::Config("Cannot determine app data directory".into()))
    }

    fn default_config(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            provider: ProviderConfig::Http {
                base_url: "https://localhost/artifacts".into(),
            },
            auth: AuthConfig::None,
            cache: CacheConfig { max_size_mb: 500 },
            telemetry: TelemetryConfig {
                enabled: true,
                remote_endpoint: None,
            },
            offline_mode: false,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { max_size_mb: 500 }
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            remote_endpoint: None,
        }
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

        let config = LauncherConfig {
            base_dir: base_dir.clone(),
            provider: ProviderConfig::GitHub {
                owner: "test-org".into(),
                repo: "tools".into(),
            },
            auth: AuthConfig::None,
            cache: CacheConfig { max_size_mb: 200 },
            telemetry: TelemetryConfig {
                enabled: false,
                remote_endpoint: None,
            },
            offline_mode: false,
        };

        config.save().unwrap();

        let config_path = base_dir.join("config").join("launcher.json");
        let loaded = LauncherConfig::load_from(&config_path).unwrap();

        assert_eq!(loaded.cache.max_size_mb, 200);
        assert!(!loaded.telemetry.enabled);
    }

    #[test]
    fn test_directory_paths() {
        let config = LauncherConfig {
            base_dir: PathBuf::from("/app"),
            provider: ProviderConfig::Http {
                base_url: String::new(),
            },
            auth: AuthConfig::None,
            cache: CacheConfig::default(),
            telemetry: TelemetryConfig::default(),
            offline_mode: false,
        };

        assert!(config.cache_dir().ends_with("cache"));
        assert!(config.downloads_dir().ends_with("downloads"));
        assert!(config.logs_dir().ends_with("logs"));
        assert!(config.metadata_dir().ends_with("metadata"));
    }

    #[test]
    fn test_provider_serialization() {
        let github = ProviderConfig::GitHub {
            owner: "org".into(),
            repo: "repo".into(),
        };
        let json = serde_json::to_string(&github).unwrap();
        assert!(json.contains("github"));

        let http = ProviderConfig::Http {
            base_url: "https://example.com".into(),
        };
        let json = serde_json::to_string(&http).unwrap();
        assert!(json.contains("http"));
    }

    #[test]
    fn test_auth_serialization() {
        let none = AuthConfig::None;
        let json = serde_json::to_string(&none).unwrap();
        assert!(json.contains("none"));

        let api_key = AuthConfig::ApiKey {
            key: "secret".into(),
        };
        let json = serde_json::to_string(&api_key).unwrap();
        assert!(json.contains("api_key"));
    }
}
