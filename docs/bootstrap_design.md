# Bootstrap Design

## Overview

The bootstrap is a minimal, stable executable responsible for maintaining and launching the main launcher application.

## Responsibilities

1. Load configuration from `%APPDATA%/AutomationLauncher/config/bootstrap.json`
2. Check remote kill-switch (if configured)
3. Check for launcher updates
4. Download and verify updated launcher binary
5. Launch the main launcher application

## Kill-Switch Mechanism

When a kill-switch URL is configured, the bootstrap checks it on every startup. If the kill-switch is active:
- The launcher binary is deleted from disk
- An optional administrator message is displayed
- The bootstrap exits without launching

This ensures compromised or deprecated versions can be remotely disabled.

## Update Process

1. Fetch update metadata from configured URL
2. Compare remote version with locally stored version
3. If update needed: download new binary, verify SHA-256 checksum
4. Replace launcher binary and update version record
5. Launch updated application

## Configuration

```json
{
  "launcher_path": "C:\\Users\\...\\AutomationLauncher\\launcher.exe",
  "update_url": "https://releases.example.com/update/metadata.json",
  "killswitch_url": "https://releases.example.com/killswitch.json",
  "base_dir": "C:\\Users\\...\\AutomationLauncher"
}
```

## Design Principles

- Must remain extremely stable and rarely updated
- Minimal dependencies
- Graceful degradation (proceeds if kill-switch check fails)
- All downloaded binaries verified before use
