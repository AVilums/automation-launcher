mod github;
mod http;
mod mock;

pub use github::GitHubProvider;
pub use http::HttpProvider;

#[cfg(test)]
pub use mock::MockProvider;

use artifact::ArtifactManifest;
use domain::LauncherError;

pub trait ArtifactProvider: Send + Sync {
    fn fetch_manifest(
        &self,
    ) -> impl std::future::Future<Output = Result<ArtifactManifest, LauncherError>> + Send;
}
