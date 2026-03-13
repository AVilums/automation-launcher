use artifact::{Artifact, ArtifactManifest, ArtifactVersion, LaunchConfig};
use domain::LauncherError;
use serde::Deserialize;
use tracing::info;

use super::ArtifactProvider;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

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
            return Err(LauncherError::Provider(format!(
                "GitHub API returned {} for {}",
                status, url
            )));
        }

        let releases: Vec<GitHubRelease> = response.json().await?;

        let mut artifacts_map: std::collections::HashMap<String, Vec<ArtifactVersion>> =
            std::collections::HashMap::new();
        let mut descriptions: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for rel in releases {
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
                format!("{}.exe", name)
            };

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

            artifacts_map
                .entry(name.clone())
                .or_default()
                .push(version_data);
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
        if let Some(manifest) = self.try_manifest_file().await {
            info!("Using manifest.json from repository");
            return Ok(manifest);
        }
        info!("Building manifest from GitHub Releases API");
        self.manifest_from_releases().await
    }
}
