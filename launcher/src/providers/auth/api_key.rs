use tracing::info;

use crate::error::LauncherError;

use super::{AuthProvider, AuthToken};

/// API Key authentication — simple token-based mechanism.
pub struct ApiKeyAuth {
    api_key: String,
}

impl ApiKeyAuth {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl AuthProvider for ApiKeyAuth {
    fn name(&self) -> &str {
        "api_key"
    }

    async fn authenticate(&self) -> Result<AuthToken, LauncherError> {
        if self.api_key.is_empty() {
            return Err(LauncherError::Auth("API key is empty".into()));
        }
        info!("Authenticated with API key");
        Ok(AuthToken::bearer(self.api_key.clone()))
    }

    fn is_authenticated(&self) -> bool {
        !self.api_key.is_empty()
    }
}
