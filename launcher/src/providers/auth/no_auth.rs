use crate::error::LauncherError;

use super::{AuthProvider, AuthToken};

/// No authentication — for internal or local deployments.
pub struct NoAuth;

impl NoAuth {
    pub fn new() -> Self {
        Self
    }
}

impl AuthProvider for NoAuth {
    fn name(&self) -> &str {
        "none"
    }

    async fn authenticate(&self) -> Result<AuthToken, LauncherError> {
        Ok(AuthToken {
            token: String::new(),
            token_type: "None".to_string(),
            expires_at: None,
        })
    }

    fn is_authenticated(&self) -> bool {
        true
    }
}
