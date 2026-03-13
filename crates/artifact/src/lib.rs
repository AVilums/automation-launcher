use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub versions: Vec<ArtifactVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    pub version: String,
    pub download_url: String,
    pub file_size: u64,
    pub sha256: String,
    pub launch: LaunchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchConfig {
    pub executable: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

/// A lightweight manifest shipped inside artifact ZIPs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalArtifactManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub entry_point: Option<String>,
}

impl ArtifactManifest {
    pub fn find_artifact(&self, name: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.name == name)
    }

    pub fn search(&self, query: &str) -> Vec<&Artifact> {
        let query_lower = query.to_lowercase();
        self.artifacts
            .iter()
            .filter(|a| {
                a.name.to_lowercase().contains(&query_lower)
                    || a.description.to_lowercase().contains(&query_lower)
                    || a.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

impl Artifact {
    pub fn latest_version(&self) -> Option<&ArtifactVersion> {
        self.versions.first()
    }

    pub fn find_version(&self, version: &str) -> Option<&ArtifactVersion> {
        self.versions.iter().find(|v| v.version == version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ArtifactManifest {
        ArtifactManifest {
            artifacts: vec![Artifact {
                name: "test-tool".into(),
                description: "A test tool".into(),
                tags: vec!["test".into()],
                versions: vec![
                    ArtifactVersion {
                        version: "2.0.0".into(),
                        download_url: "https://example.com/v2".into(),
                        file_size: 1024,
                        sha256: "abc".into(),
                        launch: LaunchConfig {
                            executable: "test.exe".into(),
                            args: None,
                            env: None,
                        },
                    },
                    ArtifactVersion {
                        version: "1.0.0".into(),
                        download_url: "https://example.com/v1".into(),
                        file_size: 512,
                        sha256: "def".into(),
                        launch: LaunchConfig {
                            executable: "test.exe".into(),
                            args: None,
                            env: None,
                        },
                    },
                ],
            }],
        }
    }

    #[test]
    fn test_find_artifact() {
        let manifest = sample_manifest();
        assert!(manifest.find_artifact("test-tool").is_some());
        assert!(manifest.find_artifact("missing").is_none());
    }

    #[test]
    fn test_latest_version() {
        let manifest = sample_manifest();
        let artifact = manifest.find_artifact("test-tool").unwrap();
        assert_eq!(artifact.latest_version().unwrap().version, "2.0.0");
    }

    #[test]
    fn test_find_version() {
        let manifest = sample_manifest();
        let artifact = manifest.find_artifact("test-tool").unwrap();
        assert!(artifact.find_version("1.0.0").is_some());
        assert!(artifact.find_version("9.9.9").is_none());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: ArtifactManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.artifacts.len(), 1);
        assert_eq!(deserialized.artifacts[0].versions.len(), 2);
    }
}
