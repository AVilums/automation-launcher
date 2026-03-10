# Authentication System

## Overview

Authentication is implemented through pluggable authentication providers, configurable via the launcher settings.

## Supported Methods

### No Authentication (`none`)
Suitable for internal deployments or local testing. No credentials required.

### API Key (`api_key`)
Simple token-based authentication. The key is included in request headers.

### JWT (`jwt`)
Short-lived tokens obtained from an authentication service endpoint.

## Future Methods

- **OAuth Device Flow** - Recommended for enterprise deployments; users authenticate through a browser
- Potential identity providers: GitHub, Azure AD, Okta, Auth0, Google Identity

## Configuration

```json
{
  "auth": {
    "type": "none"
  }
}
```

```json
{
  "auth": {
    "type": "api_key",
    "key": "your-api-key-here"
  }
}
```

```json
{
  "auth": {
    "type": "jwt",
    "token_url": "https://auth.example.com/token"
  }
}
```

## Design Principles

- Authentication providers must be interchangeable through configuration
- No provider-specific logic in core launcher code
- Credentials should be stored securely
