use thiserror::Error;

#[derive(Error, Debug)]
pub enum LauncherError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Download error: {0}")]
    Download(String),

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

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Logging error: {0}")]
    Logging(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Runtime error: {0}")]
    Runtime(String),
}
