use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalArtifactManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub entry_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub versions: Vec<ArtifactVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactVersion {
    pub version: String,
    pub download_url: String,
    pub file_size: u64,
    pub sha256: String,
    pub launch: LaunchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchConfig {
    pub executable: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub artifacts: Vec<Artifact>,
}

impl Artifact {
    pub fn latest_version(&self) -> Option<&ArtifactVersion> {
        self.versions.first()
    }

    pub fn find_version(&self, version: &str) -> Option<&ArtifactVersion> {
        self.versions.iter().find(|v| v.version == version)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_artifact() -> Artifact {
        Artifact {
            name: "test-tool".into(),
            description: "A test automation tool".into(),
            tags: vec!["testing".into(), "automation".into()],
            versions: vec![
                ArtifactVersion {
                    version: "2.0.0".into(),
                    download_url: "https://example.com/test-tool-2.0.0.zip".into(),
                    file_size: 1024,
                    sha256: "abc123".into(),
                    launch: LaunchConfig {
                        executable: "test-tool.exe".into(),
                        args: Some(vec!["--verbose".into()]),
                        env: None,
                    },
                },
                ArtifactVersion {
                    version: "1.0.0".into(),
                    download_url: "https://example.com/test-tool-1.0.0.zip".into(),
                    file_size: 512,
                    sha256: "def456".into(),
                    launch: LaunchConfig {
                        executable: "test-tool.exe".into(),
                        args: None,
                        env: None,
                    },
                },
            ],
        }
    }

    #[test]
    fn test_latest_version() {
        let artifact = sample_artifact();
        let latest = artifact.latest_version().unwrap();
        assert_eq!(latest.version, "2.0.0");
    }

    #[test]
    fn test_find_version() {
        let artifact = sample_artifact();
        let v1 = artifact.find_version("1.0.0").unwrap();
        assert_eq!(v1.file_size, 512);

        assert!(artifact.find_version("3.0.0").is_none());
    }

    #[test]
    fn test_manifest_search() {
        let manifest = ArtifactManifest {
            artifacts: vec![
                sample_artifact(),
                Artifact {
                    name: "deploy-helper".into(),
                    description: "Deployment utility".into(),
                    tags: vec!["deploy".into()],
                    versions: vec![],
                },
            ],
        };

        assert_eq!(manifest.search("test").len(), 1);
        assert_eq!(manifest.search("deploy").len(), 1);
        assert_eq!(manifest.search("automation").len(), 1);
        assert_eq!(manifest.search("nonexistent").len(), 0);
    }

    #[test]
    fn test_find_artifact() {
        let manifest = ArtifactManifest {
            artifacts: vec![sample_artifact()],
        };

        assert!(manifest.find_artifact("test-tool").is_some());
        assert!(manifest.find_artifact("missing").is_none());
    }

    #[test]
    fn test_artifact_serialization() {
        let artifact = sample_artifact();
        let json = serde_json::to_string(&artifact).unwrap();
        let deserialized: Artifact = serde_json::from_str(&json).unwrap();
        assert_eq!(artifact, deserialized);
    }

    #[test]
    fn test_launch_config_with_env() {
        let mut env = HashMap::new();
        env.insert("PATH".into(), "/usr/bin".into());

        let config = LaunchConfig {
            executable: "tool.exe".into(),
            args: Some(vec!["--run".into()]),
            env: Some(env),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: LaunchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
