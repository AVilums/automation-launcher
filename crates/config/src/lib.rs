use domain::LauncherError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderConfig {
    #[serde(rename = "github")]
    GitHub { owner: String, repo: String },
    #[serde(rename = "http")]
    Http { base_url: String },
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig::GitHub {
            owner: "org-on-auto".into(),
            repo: "automations".into(),
        }
    }
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

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_mb: default_max_size_mb(),
        }
    }
}

fn default_max_size_mb() -> u64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_telemetry_enabled")]
    pub enabled: bool,
    pub remote_endpoint: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: default_telemetry_enabled(),
            remote_endpoint: None,
        }
    }
}

fn default_telemetry_enabled() -> bool {
    true
}

fn default_base_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("AutomationLauncher")
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            base_dir: default_base_dir(),
            provider: ProviderConfig::default(),
            auth: AuthConfig::default(),
            offline_mode: false,
            cache: CacheConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

impl LauncherConfig {
    pub fn load() -> Result<Self, LauncherError> {
        let config_path = Self::default_config_path();
        if config_path.exists() {
            Self::load_from(&config_path)
        } else {
            info!("No config file found, using defaults");
            Ok(Self::default())
        }
    }

    pub fn load_from(path: &Path) -> Result<Self, LauncherError> {
        let content = std::fs::read_to_string(path)?;
        let config: LauncherConfig = serde_json::from_str(&content)?;
        info!("Loaded config from {}", path.display());
        Ok(config)
    }

    pub fn save(&self) -> Result<(), LauncherError> {
        let path = Self::default_config_path();
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), LauncherError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn default_config_path() -> PathBuf {
        default_base_dir().join("config").join("launcher.json")
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LauncherConfig::default();
        assert!(!config.offline_mode);
        assert_eq!(config.cache.max_size_mb, 500);
        assert!(config.telemetry.enabled);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = LauncherConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: LauncherConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cache.max_size_mb, config.cache.max_size_mb);
    }
}
