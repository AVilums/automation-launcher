# Artifact Model

## Overview

Automation tools distributed through the launcher follow a manifest-driven model. The artifact model is provider-agnostic.

## Structures

### Artifact
Represents a tool with metadata:
- `name` - Unique tool identifier
- `description` - Human-readable description
- `tags` - Categorization tags for search/filter
- `versions` - List of available versions

### ArtifactVersion
Each version includes:
- `version` - Version identifier (e.g., "1.2.0")
- `download_url` - URL to download the artifact archive
- `file_size` - Expected file size in bytes
- `sha256` - SHA-256 checksum for integrity verification
- `launch` - Launch configuration

### LaunchConfig
Specifies how to execute the tool:
- `executable` - Executable filename (e.g., "tool.exe")
- `args` - Optional default arguments
- `env` - Optional environment variables

### ArtifactManifest
Container for all available artifacts:
- `artifacts` - List of Artifact entries
- Supports search by name, description, and tags
- Supports lookup by artifact name

## Example Manifest

```json
{
  "artifacts": [
    {
      "name": "data-processor",
      "description": "Automated data processing tool",
      "tags": ["data", "automation"],
      "versions": [
        {
          "version": "2.0.0",
          "download_url": "https://releases.example.com/data-processor-2.0.0.zip",
          "file_size": 5242880,
          "sha256": "a1b2c3d4...",
          "launch": {
            "executable": "data-processor.exe",
            "args": ["--config", "default.json"],
            "env": { "LOG_LEVEL": "info" }
          }
        }
      ]
    }
  ]
}
```
