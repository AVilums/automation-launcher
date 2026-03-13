use config::AuthConfig;
use domain::LauncherError;

/// Resolve the bearer token (if any) from the auth configuration.
pub fn resolve_token(auth: &AuthConfig) -> Result<Option<String>, LauncherError> {
    match auth {
        AuthConfig::None => Ok(None),
        AuthConfig::ApiKey { key } => Ok(Some(key.clone())),
        AuthConfig::Jwt { token_url } => {
            tracing::warn!("JWT auth not yet implemented (url: {})", token_url);
            Err(LauncherError::Auth(
                "JWT authentication is not yet implemented".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_none() {
        let token = resolve_token(&AuthConfig::None).unwrap();
        assert!(token.is_none());
    }

    #[test]
    fn test_resolve_api_key() {
        let token = resolve_token(&AuthConfig::ApiKey {
            key: "secret".into(),
        })
        .unwrap();
        assert_eq!(token, Some("secret".into()));
    }

    #[test]
    fn test_resolve_jwt_not_implemented() {
        let result = resolve_token(&AuthConfig::Jwt {
            token_url: "https://example.com/token".into(),
        });
        assert!(result.is_err());
    }
}
