use crate::artifact::ArtifactManifest;
use crate::error::LauncherError;

use super::ArtifactProvider;

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
    use crate::artifact::{Artifact, ArtifactVersion, LaunchConfig};

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
