use serde::Deserialize;
use tracing::info;

use crate::error::LauncherError;

use super::{AuthProvider, AuthToken};

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
