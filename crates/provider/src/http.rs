use artifact::ArtifactManifest;
use domain::LauncherError;
use tracing::info;

use super::ArtifactProvider;

pub struct HttpProvider {
    pub base_url: String,
    client: reqwest::Client,
}

impl HttpProvider {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

impl ArtifactProvider for HttpProvider {
    async fn fetch_manifest(&self) -> Result<ArtifactManifest, LauncherError> {
        let url = format!("{}/manifest.json", self.base_url.trim_end_matches('/'));
        info!("Fetching manifest from {}", url);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(LauncherError::Provider(format!(
                "HTTP {} from {}",
                response.status(),
                url
            )));
        }
        let manifest: ArtifactManifest = response.json().await?;
        Ok(manifest)
    }
}
