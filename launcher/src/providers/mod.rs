#[allow(dead_code)]
pub mod auth;
mod github;
mod http;
mod mock;

use crate::artifact::ArtifactManifest;
use crate::error::LauncherError;

pub use github::GitHubProvider;
pub use http::HttpProvider;

#[cfg(test)]
pub use mock::MockProvider;

pub trait ArtifactProvider: Send + Sync {
    fn fetch_manifest(
        &self,
    ) -> impl std::future::Future<Output = Result<ArtifactManifest, LauncherError>> + Send;
}
