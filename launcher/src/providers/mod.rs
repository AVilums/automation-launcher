#[allow(dead_code)]
pub mod auth;
mod provider;

pub use provider::{ArtifactProvider, GitHubProvider, HttpProvider};

#[cfg(test)]
pub use provider::MockProvider;
