use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Artifact not found: {0}")]
    ArtifactNotFound(String),

    #[error("Version not found: {tool} v{version}")]
    VersionNotFound { tool: String, version: String },

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Authentication error: {0}")]
    #[allow(dead_code)]
    Auth(String),

    #[error("Download error: {0}")]
    Download(String),

    #[error("Logging error: {0}")]
    Logging(String),
}
