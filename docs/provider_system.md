# Provider System

## Overview

The launcher uses a provider abstraction layer to support fetching artifact manifests from different repository systems.

## ArtifactProvider Trait

All providers implement the `ArtifactProvider` trait:
- `name()` - Returns the provider identifier
- `fetch_manifest()` - Fetches the complete artifact manifest

## Implementations

### HttpProvider
Fetches manifests from a generic HTTP endpoint.
- Expects a `manifest.json` at `{base_url}/manifest.json`
- Suitable for custom artifact servers

### GitHubProvider
Fetches manifests from a GitHub repository.
- Reads `manifest.json` from the `main` branch via raw.githubusercontent.com
- Uses a custom User-Agent header

### MockProvider
Returns a pre-configured manifest for testing.
- No network calls
- Useful for unit and integration tests

## Future Providers

Planned provider implementations:
- GitLab releases
- Cloud object storage (S3, Azure Blob)
- Artifact repository managers (Artifactory, Nexus)
- Enterprise API integrations

## Configuration

Provider selection is configured in `launcher.json`:

```json
{
  "provider": {
    "type": "github",
    "owner": "my-org",
    "repo": "automation-tools"
  }
}
```

Or for HTTP:

```json
{
  "provider": {
    "type": "http",
    "base_url": "https://artifacts.example.com"
  }
}
```
