mod api_key;
mod jwt;
mod no_auth;
mod oauth_device;

use crate::config::AuthConfig;
use serde::{Deserialize, Serialize};

use crate::error::LauncherError;

pub use api_key::ApiKeyAuth;
pub use jwt::JwtAuth;
pub use no_auth::NoAuth;
#[allow(unused_imports)]
pub use oauth_device::OAuthDeviceFlow;

/// Represents an authentication token that can be attached to requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub token_type: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl AuthToken {
    pub fn bearer(token: String) -> Self {
        Self {
            token,
            token_type: "Bearer".to_string(),
            expires_at: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => chrono::Utc::now() >= expiry,
            None => false,
        }
    }

    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.token_type, self.token)
    }
}

/// Trait for authentication providers.
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &str;
    fn authenticate(
        &self,
    ) -> impl std::future::Future<Output = Result<AuthToken, LauncherError>> + Send;
    fn is_authenticated(&self) -> bool;
}

/// Create an authentication provider from configuration.
pub fn create_auth_provider(config: &AuthConfig) -> Box<dyn std::any::Any + Send + Sync> {
    match config {
        AuthConfig::None => Box::new(NoAuth::new()),
        AuthConfig::ApiKey { key } => Box::new(ApiKeyAuth::new(key.clone())),
        AuthConfig::Jwt { token_url } => Box::new(JwtAuth::new(token_url.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_token_bearer() {
        let token = AuthToken::bearer("my-secret-token".into());
        assert_eq!(token.token_type, "Bearer");
        assert_eq!(token.token, "my-secret-token");
        assert!(!token.is_expired());
        assert_eq!(
            token.authorization_header(),
            "Bearer my-secret-token"
        );
    }

    #[test]
    fn test_auth_token_expired() {
        let token = AuthToken {
            token: "expired".into(),
            token_type: "Bearer".into(),
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
        };
        assert!(token.is_expired());
    }

    #[test]
    fn test_auth_token_not_expired() {
        let token = AuthToken {
            token: "valid".into(),
            token_type: "Bearer".into(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn test_auth_token_no_expiry() {
        let token = AuthToken {
            token: "forever".into(),
            token_type: "Bearer".into(),
            expires_at: None,
        };
        assert!(!token.is_expired());
    }

    #[tokio::test]
    async fn test_no_auth() {
        let provider = NoAuth::new();
        assert_eq!(provider.name(), "none");
        assert!(provider.is_authenticated());

        let token = provider.authenticate().await.unwrap();
        assert_eq!(token.token_type, "None");
        assert!(token.token.is_empty());
    }

    #[tokio::test]
    async fn test_api_key_auth() {
        let provider = ApiKeyAuth::new("my-api-key".into());
        assert_eq!(provider.name(), "api_key");
        assert!(provider.is_authenticated());

        let token = provider.authenticate().await.unwrap();
        assert_eq!(token.token, "my-api-key");
        assert_eq!(token.token_type, "Bearer");
    }

    #[tokio::test]
    async fn test_api_key_auth_empty() {
        let provider = ApiKeyAuth::new(String::new());
        assert!(!provider.is_authenticated());

        let result = provider.authenticate().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_auth_not_authenticated_initially() {
        let provider = JwtAuth::new("https://example.com/token".into());
        assert_eq!(provider.name(), "jwt");
        assert!(!provider.is_authenticated());
    }

    #[test]
    fn test_oauth_device_flow_not_authenticated_initially() {
        let provider = OAuthDeviceFlow::new(
            "https://example.com/device".into(),
            "https://example.com/token".into(),
            "client-123".into(),
        );
        assert_eq!(provider.name(), "oauth_device_flow");
        assert!(!provider.is_authenticated());
    }

    #[test]
    fn test_create_auth_provider_none() {
        let config = AuthConfig::None;
        let provider = create_auth_provider(&config);
        let no_auth = provider.downcast_ref::<NoAuth>();
        assert!(no_auth.is_some());
    }

    #[test]
    fn test_create_auth_provider_api_key() {
        let config = AuthConfig::ApiKey {
            key: "secret".into(),
        };
        let provider = create_auth_provider(&config);
        let api_auth = provider.downcast_ref::<ApiKeyAuth>();
        assert!(api_auth.is_some());
    }
}
