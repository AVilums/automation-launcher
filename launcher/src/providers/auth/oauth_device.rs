use serde::Deserialize;
use tracing::info;

use crate::error::LauncherError;

use super::{AuthProvider, AuthToken};

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
