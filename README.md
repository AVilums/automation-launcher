# Automation Launcher

A secure, modular tool distribution and execution platform built in Rust. Browse, download, cache, and run automation tools from GitHub Releases or HTTP endpoints — with a full GUI and a headless runner for scheduling.

---

## Features

- **Browse & Search** — Discover automation tools from a configured GitHub repo or HTTP manifest
- **Download & Cache** — Download artifacts with resume support, SHA-256 verification, and LRU cache eviction
- **Run Tools** — Execute `.exe`, `.ps1`, and `.bat` files with configurable launch parameters
- **GUI (egui)** — Native desktop interface with Browse, Favorites, Cache, and Settings tabs
- **CLI** — Full command-line interface for all operations
- **Runner Binary** — Headless tool execution with built-in task scheduling (interval, daily, one-shot)
- **Favorites** — Pin frequently used tools for quick access
- **Offline Mode** — Work with cached artifacts when disconnected
- **Telemetry** — Local JSONL event logging with optional remote batch flushing
- **Modular Architecture** — 11 workspace crates with clear separation of concerns

---

## Architecture

```
automation-launcher/
├── launcher/          # Main binary (CLI + GUI)
├── runner/            # Headless runner binary (scheduling + execution)
├── bootstrap/         # Bootstrap installer
└── crates/
    ├── domain/        # Shared error types and utilities
    ├── artifact/      # Artifact manifest types and search
    ├── config/        # Configuration loading/saving
    ├── network/       # Download manager with resume + checksum
    ├── auth/          # Authentication (API key, JWT stub)
    ├── storage/       # Cache manager, ZIP extraction, favorites
    ├── telemetry/     # Event logging and tracing setup
    ├── provider/      # GitHub and HTTP manifest providers
    ├── execution/     # Process execution (exe/ps1/bat)
    ├── runtime/       # Orchestration (fetch → download → cache → run)
    └── scheduler/     # Task scheduling (interval/daily/once)
```

---

## Quick Start

### Prerequisites

- **Rust 1.85+** (edition 2024)
- **Windows** (primary target; PowerShell scripts and `.exe` execution)

### Build

```powershell
# Clone and build everything
cd automation-launcher

# Build with GUI (default)
cargo build --release

# Build without GUI
cargo build --release --no-default-features
```

### Run the Launcher (GUI)

```powershell
# Opens the graphical interface
cargo run --release

# Or run the built binary
.\target\release\launcher.exe
```

### Run the Launcher (CLI)

```powershell
# Show status
cargo run --release -- --no-default-features

# List available tools
launcher list

# Search for tools
launcher search "sort"

# Download a tool
launcher download my-tool --version 1.0.0

# Run a cached tool
launcher run my-tool --version 1.0.0 --wait

# Cache management
launcher cache status
launcher cache list
launcher cache remove my-tool 1.0.0
launcher cache clear

# Favorites
launcher fav list
launcher fav add my-tool --version 1.0.0
launcher fav remove my-tool

# Configuration
launcher config show
launcher config path
launcher config reset
```

### Run the Runner (Headless)

```powershell
# Run a tool directly
runner run my-tool --wait

# Schedule a task (run every 300 seconds)
runner schedule add --id daily-sort --interval 300 sort-folders

# List scheduled tasks
runner schedule list

# Run all due tasks once
runner tick

# Start the scheduler daemon
runner daemon --interval 60
```

---

## Configuration

Configuration is stored at `%LOCALAPPDATA%\AutomationLauncher\config\launcher.json`.

```json
{
  "base_dir": "C:\\Users\\you\\AppData\\Local\\AutomationLauncher",
  "provider": {
    "type": "github",
    "owner": "orgonautomation",
    "repo": "automations"
  },
  "auth": {
    "type": "api_key",
    "key": "ghp_your_token_here"
  },
  "offline_mode": false,
  "cache": {
    "max_size_mb": 500
  },
  "telemetry": {
    "enabled": true,
    "remote_endpoint": null
  }
}
```

### Provider Types

| Type | Config |
|------|--------|
| GitHub | `{ "type": "github", "owner": "...", "repo": "..." }` |
| HTTP | `{ "type": "http", "base_url": "https://..." }` |

### Auth Types

| Type | Config |
|------|--------|
| None | `{ "type": "none" }` |
| API Key | `{ "type": "api_key", "key": "..." }` |
| JWT | `{ "type": "jwt", "token_url": "..." }` (stub) |

---

## Testing

```powershell
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p storage
cargo test -p scheduler
```

---

## What's Next

- **JWT authentication** — Complete the JWT token exchange flow
- **Auto-update** — Check for new tool versions and update cached artifacts
- **Plugin system** — Support custom execution runtimes (Python, Node, etc.)
- **Notifications** — Desktop notifications for scheduled task results
- **Multi-platform** — Linux/macOS support for execution formats
- **Manifest editor** — GUI for creating and editing `manifest.json` files
- **Dependency resolution** — Tools that depend on other tools
- **Rollback** — Revert to previous tool versions on failure
- **Audit log** — Detailed execution history with stdout/stderr capture
- **Remote dashboard** — Web UI for monitoring fleet-wide tool execution
