use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config::AuthConfig;
use crate::error::LauncherError;

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

/// JWT authentication — obtains short-lived tokens from an auth service.
pub struct JwtAuth {
    token_url: String,
    client: reqwest::Client,
    cached_token: std::sync::Mutex<Option<AuthToken>>,
}

impl JwtAuth {
    pub fn new(token_url: String) -> Self {
        Self {
            token_url,
            client: reqwest::Client::new(),
            cached_token: std::sync::Mutex::new(None),
        }
    }
}

/// JWT token response from the authentication server.
#[derive(Debug, Deserialize)]
struct JwtTokenResponse {
    access_token: String,
    expires_in: Option<i64>,
}

impl AuthProvider for JwtAuth {
    fn name(&self) -> &str {
        "jwt"
    }

    async fn authenticate(&self) -> Result<AuthToken, LauncherError> {
        // Check cached token
        if let Ok(guard) = self.cached_token.lock() {
            if let Some(ref token) = *guard {
                if !token.is_expired() {
                    return Ok(token.clone());
                }
            }
        }

        info!("Requesting JWT token from {}", self.token_url);
        let response = self
            .client
            .post(&self.token_url)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| LauncherError::Auth(format!("JWT request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(LauncherError::Auth(format!(
                "JWT auth failed with status {}",
                response.status()
            )));
        }

        let jwt_response: JwtTokenResponse = response
            .json()
            .await
            .map_err(|e| LauncherError::Auth(format!("Failed to parse JWT response: {}", e)))?;

        let expires_at = jwt_response.expires_in.map(|secs| {
            chrono::Utc::now() + chrono::Duration::seconds(secs)
        });

        let token = AuthToken {
            token: jwt_response.access_token,
            token_type: "Bearer".to_string(),
            expires_at,
        };

        if let Ok(mut guard) = self.cached_token.lock() {
            *guard = Some(token.clone());
        }

        info!("JWT token obtained successfully");
        Ok(token)
    }

    fn is_authenticated(&self) -> bool {
        if let Ok(guard) = self.cached_token.lock() {
            guard.as_ref().is_some_and(|t| !t.is_expired())
        } else {
            false
        }
    }
}

/// OAuth Device Flow — browser-based authentication for enterprise deployments.
///
/// Flow:
/// 1. Request device code from authorization server
/// 2. Display user code and verification URL to the user
/// 3. Poll for token until user completes browser authentication
pub struct OAuthDeviceFlow {
    device_auth_url: String,
    token_url: String,
    client_id: String,
    client: reqwest::Client,
    cached_token: std::sync::Mutex<Option<AuthToken>>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    expires_in: Option<i64>,
}

impl OAuthDeviceFlow {
    pub fn new(device_auth_url: String, token_url: String, client_id: String) -> Self {
        Self {
            device_auth_url,
            token_url,
            client_id,
            client: reqwest::Client::new(),
            cached_token: std::sync::Mutex::new(None),
        }
    }
}

impl AuthProvider for OAuthDeviceFlow {
    fn name(&self) -> &str {
        "oauth_device_flow"
    }

    async fn authenticate(&self) -> Result<AuthToken, LauncherError> {
        // Check cached token
        if let Ok(guard) = self.cached_token.lock() {
            if let Some(ref token) = *guard {
                if !token.is_expired() {
                    return Ok(token.clone());
                }
            }
        }

        // Step 1: Request device code
        info!("Requesting device code from {}", self.device_auth_url);
        let device_response = self
            .client
            .post(&self.device_auth_url)
            .form(&[("client_id", &self.client_id)])
            .send()
            .await
            .map_err(|e| LauncherError::Auth(format!("Device code request failed: {}", e)))?;

        let device_code: DeviceCodeResponse = device_response
            .json()
            .await
            .map_err(|e| LauncherError::Auth(format!("Failed to parse device code: {}", e)))?;

        // Step 2: Display instructions to user
        println!();
        println!("=== Authentication Required ===");
        println!("Open this URL in your browser: {}", device_code.verification_uri);
        println!("Enter this code: {}", device_code.user_code);
        println!("Waiting for authentication...");
        println!();

        // Step 3: Poll for token
        let interval = std::time::Duration::from_secs(device_code.interval.unwrap_or(5));
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(device_code.expires_in);

        while std::time::Instant::now() < deadline {
            tokio::time::sleep(interval).await;

            let token_response = self
                .client
                .post(&self.token_url)
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("device_code", device_code.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| LauncherError::Auth(format!("Token poll failed: {}", e)))?;

            let oauth_response: OAuthTokenResponse = token_response
                .json()
                .await
                .map_err(|e| LauncherError::Auth(format!("Failed to parse token: {}", e)))?;

            if let Some(access_token) = oauth_response.access_token {
                let expires_at = oauth_response.expires_in.map(|secs| {
                    chrono::Utc::now() + chrono::Duration::seconds(secs)
                });

                let token = AuthToken {
                    token: access_token,
                    token_type: "Bearer".to_string(),
                    expires_at,
                };

                if let Ok(mut guard) = self.cached_token.lock() {
                    *guard = Some(token.clone());
                }

                println!("Authentication successful!");
                info!("OAuth device flow authentication completed");
                return Ok(token);
            }

            match oauth_response.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                Some(err) => {
                    return Err(LauncherError::Auth(format!("OAuth error: {}", err)));
                }
                None => continue,
            }
        }

        Err(LauncherError::Auth(
            "Device flow authentication timed out".into(),
        ))
    }

    fn is_authenticated(&self) -> bool {
        if let Ok(guard) = self.cached_token.lock() {
            guard.as_ref().is_some_and(|t| !t.is_expired())
        } else {
            false
        }
    }
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
