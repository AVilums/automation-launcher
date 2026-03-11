use serde::Deserialize;
use tracing::info;

use crate::artifact::{Artifact, ArtifactManifest, ArtifactVersion, LaunchConfig};
use crate::error::LauncherError;

pub trait ArtifactProvider: Send + Sync {
    fn fetch_manifest(
        &self,
    ) -> impl std::future::Future<Output = Result<ArtifactManifest, LauncherError>> + Send;
}

// ---------------------------------------------------------------------------
// HTTP Provider — fetches manifest.json from a base URL
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// GitHub Releases Provider — discovers artifacts from GitHub Releases API
// ---------------------------------------------------------------------------

/// A single GitHub release as returned by the API.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

/// A single asset attached to a GitHub release.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    #[allow(dead_code)]
    id: u64,
    name: String,
    size: u64,
    url: String,
    browser_download_url: String,
}

pub struct GitHubProvider {
    pub owner: String,
    pub repo: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl GitHubProvider {
    pub fn new(owner: String, repo: String, token: Option<String>) -> Self {
        Self {
            owner,
            repo,
            token,
            client: reqwest::Client::builder()
                .user_agent("AutomationLauncher/0.1")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Try to fetch a manifest.json from the repo root first.
    /// If that fails, fall back to building a manifest from GitHub Releases.
    async fn try_manifest_file(&self) -> Option<ArtifactManifest> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/main/manifest.json",
            self.owner, self.repo
        );
        let mut builder = self.client.get(&url);
        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token);
        }
        let resp = builder.send().await.ok()?;
        if !resp.status().is_success() {
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                tracing::debug!("Manifest not found at {}: 404", url);
            } else {
                tracing::warn!("Failed to fetch manifest from {}: {}", url, resp.status());
            }
            return None;
        }
        resp.json::<ArtifactManifest>().await.ok()
    }

    /// Build a manifest by querying the GitHub Releases API.
    async fn manifest_from_releases(&self) -> Result<ArtifactManifest, LauncherError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases",
            self.owner, self.repo
        );
        info!("Fetching GitHub releases from {}", url);

        let mut builder = self.client.get(&url);
        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token);
        }
        let response = builder.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_msg = format!("GitHub API returned {} for {}", status, url);
            tracing::error!("{}", error_msg);
            return Err(LauncherError::Provider(error_msg));
        }

        let releases: Vec<GitHubRelease> = response.json().await?;

        let mut artifacts_map: std::collections::HashMap<String, Vec<ArtifactVersion>> =
            std::collections::HashMap::new();
        let mut descriptions: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for rel in releases {
            // Pick the first downloadable asset (zip/exe/tar.gz)
            let asset = match rel.assets.iter().find(|a| {
                let n = a.name.to_lowercase();
                n.ends_with(".zip")
                    || n.ends_with(".exe")
                    || n.ends_with(".tar.gz")
                    || n.ends_with(".msi")
            }) {
                Some(a) => a,
                None => continue,
            };

            // Parse automation name and version from tag name (e.g., sort-folders-v1.0.8)
            // or if it's just v1.0.8, use the repo name
            let (name, version) = if let Some(pos) = rel.tag_name.rfind("-v") {
                let (n, v) = rel.tag_name.split_at(pos);
                (n.to_string(), v.trim_start_matches("-v").to_string())
            } else {
                (
                    self.repo.clone(),
                    rel.tag_name.trim_start_matches('v').to_string(),
                )
            };

            let executable = if asset.name.ends_with(".exe") {
                asset.name.clone()
            } else {
                // Use the automation name as the expected executable name
                format!("{}.exe", name)
            };

            // If a token is provided, use the API URL for assets to support private repositories
            let download_url = if self.token.is_some() {
                asset.url.clone()
            } else {
                asset.browser_download_url.clone()
            };

            let version_data = ArtifactVersion {
                version,
                download_url,
                file_size: asset.size,
                sha256: String::new(),
                launch: LaunchConfig {
                    executable,
                    args: None,
                    env: None,
                },
            };

            artifacts_map.entry(name.clone()).or_default().push(version_data);
            if let Some(body) = rel.body {
                descriptions.entry(name).or_insert(body);
            }
        }

        let artifacts: Vec<Artifact> = artifacts_map
            .into_iter()
            .map(|(name, versions)| Artifact {
                description: descriptions.get(&name).cloned().unwrap_or_default(),
                name,
                tags: vec!["github".into()],
                versions,
            })
            .collect();

        Ok(ArtifactManifest { artifacts })
    }
}

impl ArtifactProvider for GitHubProvider {
    async fn fetch_manifest(&self) -> Result<ArtifactManifest, LauncherError> {
        // Prefer an explicit manifest.json in the repo
        if let Some(manifest) = self.try_manifest_file().await {
            info!("Using manifest.json from repository");
            return Ok(manifest);
        }
        // Fall back to building from releases
        info!("Building manifest from GitHub Releases API");
        self.manifest_from_releases().await
    }
}

// ---------------------------------------------------------------------------
// Mock Provider — for testing
// ---------------------------------------------------------------------------

pub struct MockProvider {
    manifest: ArtifactManifest,
}

impl MockProvider {
    #[allow(dead_code)]
    pub fn new(manifest: ArtifactManifest) -> Self {
        Self { manifest }
    }
}

impl ArtifactProvider for MockProvider {
    async fn fetch_manifest(&self) -> Result<ArtifactManifest, LauncherError> {
        Ok(self.manifest.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ArtifactManifest {
        ArtifactManifest {
            artifacts: vec![Artifact {
                name: "mock-tool".into(),
                description: "A mock tool".into(),
                tags: vec!["test".into()],
                versions: vec![ArtifactVersion {
                    version: "1.0.0".into(),
                    download_url: "https://example.com/mock-tool-1.0.0.zip".into(),
                    file_size: 256,
                    sha256: "aabbcc".into(),
                    launch: LaunchConfig {
                        executable: "mock.exe".into(),
                        args: None,
                        env: None,
                    },
                }],
            }],
        }
    }

    #[tokio::test]
    async fn test_mock_provider() {
        let manifest = sample_manifest();
        let provider = MockProvider::new(manifest.clone());

        let fetched = provider.fetch_manifest().await.unwrap();
        assert_eq!(fetched.artifacts.len(), 1);
        assert_eq!(fetched.artifacts[0].name, "mock-tool");
    }

}
