# Architecture Overview

The Automation Launcher follows a layered architecture model with clear separation of responsibilities.

## System Components

The system consists of two executables:

### Bootstrap Application
A minimal executable responsible for maintaining and launching the main launcher.
- Verifies launcher installation
- Checks for launcher updates
- Downloads and validates updated launcher binaries
- Enforces remote kill-switch rules
- Launches the main application

### Launcher Application
The primary user-facing application.
- Artifact discovery and browsing
- Tool execution
- Artifact downloads and verification
- Local caching
- Authentication
- Telemetry logging

## Layered Architecture

```
User Interface Layer
        ↓
Application Core
        ↓
Artifact Management
        ↓
Provider Abstraction
        ↓
Download and Verification
        ↓
Local Storage
```

## Workspace Structure

```
automation-launcher/
    bootstrap/       - Bootstrap executable
    launcher/        - Main launcher executable
    docs/            - Documentation
```

## Launcher Module Organization

- `config` - Configuration management
- `artifact` - Artifact model and manifest
- `provider` - Artifact provider abstraction (HTTP, GitHub, Mock)
- `download` - Download management with retry and checksum verification
- `cache` - Local artifact caching with LRU eviction
- `execution` - Process execution for .exe, .ps1, .bat formats
- `telemetry` - Structured telemetry event logging
- `logging` - Application logging setup
- `error` - Structured error types
