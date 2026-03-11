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
    name: String,
    size: u64,
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
            return Err(LauncherError::Provider(format!(
                "GitHub API returned {}",
                response.status()
            )));
        }

        let releases: Vec<GitHubRelease> = response.json().await?;

        // Group releases into a single artifact named after the repo.
        let versions: Vec<ArtifactVersion> = releases
            .iter()
            .filter_map(|rel| {
                // Pick the first downloadable asset (zip/exe/tar.gz)
                let asset = rel.assets.iter().find(|a| {
                    let n = a.name.to_lowercase();
                    n.ends_with(".zip")
                        || n.ends_with(".exe")
                        || n.ends_with(".tar.gz")
                        || n.ends_with(".msi")
                })?;

                let executable = if asset.name.ends_with(".exe") {
                    asset.name.clone()
                } else {
                    // Assume the archive contains an exe with the repo name
                    format!("{}.exe", self.repo)
                };

                Some(ArtifactVersion {
                    version: rel.tag_name.trim_start_matches('v').to_string(),
                    download_url: asset.browser_download_url.clone(),
                    file_size: asset.size,
                    sha256: String::new(), // GitHub doesn't provide checksums
                    launch: LaunchConfig {
                        executable,
                        args: None,
                        env: None,
                    },
                })
            })
            .collect();

        let description = releases
            .first()
            .and_then(|r| r.body.clone())
            .unwrap_or_default();

        let artifact = Artifact {
            name: self.repo.clone(),
            description,
            tags: vec!["github".into()],
            versions,
        };

        Ok(ArtifactManifest {
            artifacts: vec![artifact],
        })
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
