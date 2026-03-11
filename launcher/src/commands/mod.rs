mod cache;
mod config;
pub(crate) mod download;
mod fav;
mod list;
pub(crate) mod run;
mod search;

pub use cache::cmd_cache;
pub use config::cmd_config;
pub use download::cmd_download;
pub use fav::cmd_fav;
pub use list::cmd_list;
pub use run::cmd_run;
pub use search::cmd_search;

use crate::artifact::ArtifactManifest;
use crate::config::{LauncherConfig, ProviderConfig};
use crate::error::LauncherError;
use crate::providers::{GitHubProvider, HttpProvider, ArtifactProvider};

pub async fn fetch_manifest(config: &LauncherConfig) -> Result<ArtifactManifest, LauncherError> {
    if config.offline_mode {
        let manifest_path = config.metadata_dir().join("manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: ArtifactManifest = serde_json::from_str(&content)?;
            return Ok(manifest);
        }
        return Err(LauncherError::Provider(
            "Offline mode: no cached manifest available".into(),
        ));
    }

    let manifest = match &config.provider {
        ProviderConfig::GitHub { owner, repo } => {
            let token = match &config.auth {
                crate::config::AuthConfig::ApiKey { key } => Some(key.clone()),
                _ => None,
            };
            let provider = GitHubProvider::new(owner.clone(), repo.clone(), token);
            provider.fetch_manifest().await?
        }
        ProviderConfig::Http { base_url } => {
            let provider = HttpProvider::new(base_url.clone());
            provider.fetch_manifest().await?
        }
    };

    // Cache the manifest for offline use
    let metadata_dir = config.metadata_dir();
    std::fs::create_dir_all(&metadata_dir)?;
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(metadata_dir.join("manifest.json"), manifest_json)?;

    Ok(manifest)
}

use crate::util::format_bytes;
