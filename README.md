# Automation Launcher

A **Windows-first application** for securely distributing, managing, and executing automation tools and internal operational software. Built in Rust for performance, safety, and reliability.

---

## Features

- **Artifact Management** — Browse, search, download, and cache automation tools from remote repositories
- **Secure Execution** — SHA256 checksum verification before any artifact runs; supports `.exe`, `.ps1`, and `.bat` formats
- **Provider Abstraction** — Pluggable providers for GitHub Releases and generic HTTP repositories
- **Authentication** — Pluggable auth: None, API Key, JWT, and OAuth Device Flow
- **Caching** — LRU-based local cache with configurable size limits and offline mode support
- **Telemetry** — Structured event logging (local files + optional remote endpoint with batching)
- **Bootstrap Updater** — Separate bootstrap binary that auto-updates the launcher, with remote kill-switch support
- **GUI** — Graphical interface built with egui/eframe (enabled by default)
- **Favorites** — Pin frequently used tools for quick access
- **CI/CD** — GitHub Actions workflow for build, test, and release

---

## Prerequisites

- **Rust toolchain** (1.85+ recommended, edition 2024) — install via [rustup](https://rustup.rs/)
- **Windows 10/11** (primary target; may work on other platforms for CLI mode)
- **Git** (for cloning the repository)

---

## Quick Start

### 1. Clone the repository

```bash
git clone https://github.com/your-org/automation-launcher.git
cd automation-launcher
```

### 2. Build the project

```bash
# Build both bootstrap and launcher (debug)
cargo build

# Build in release mode
cargo build --release
```

### 3. Run tests

```bash
cargo test --all
```

### 4. Run the launcher

```bash
# Show launcher info and status
cargo run --bin launcher

# Or use the compiled binary directly
./target/release/launcher.exe
```

---

## Usage

### CLI Commands

```
launcher [OPTIONS] [COMMAND]

Options:
  --config <PATH>    Path to a custom configuration file
  --offline          Run in offline mode (use cached artifacts only)
  -h, --help         Print help
  -V, --version      Print version

Commands:
  list       List available automation tools
  search     Search for tools by name, description, or tag
  download   Download a tool artifact
  run        Run a cached automation tool
  cache      Cache management commands
  config     Show or update configuration
  fav        Manage favorite tools
  gui        Launch the graphical interface
```

### Examples

```bash
# List all available tools
launcher list

# Search for tools
launcher search "data"

# Download a specific tool (latest version)
launcher download my-tool

# Download a specific version
launcher download my-tool --version 1.2.0

# Run a tool (downloads if not cached)
launcher run my-tool

# Run with specific version and extra arguments
launcher run my-tool --version 1.2.0 --wait -- --input data.csv

# Cache management
launcher cache status
launcher cache list
launcher cache remove my-tool 1.2.0
launcher cache clear

# Show current config
launcher config show
launcher config path
launcher config reset

# Favorites
launcher fav list
launcher fav add my-tool
launcher fav add my-tool --version 2.0.0
launcher fav remove my-tool

# Launch GUI (if built with --features gui)
launcher gui
```

### Bootstrap

The bootstrap binary manages launcher updates and enforces the remote kill-switch:

```bash
# Normal launch (checks for updates, then starts the launcher)
bootstrap.exe

# Skip update check
bootstrap.exe --skip-update

# Force update even if versions match
bootstrap.exe --force-update

# Check for updates without launching
bootstrap.exe --check-only
```

---

## Configuration

On first run, the launcher creates a default configuration at:

```
%APPDATA%\AutomationLauncher\config\launcher.json
```

### Configuration Options

| Setting | Description | Default |
|---------|-------------|---------|
| `provider.type` | Artifact provider: `github` or `http` | `http` |
| `provider.owner` / `provider.repo` | GitHub org and repo (for `github` type) | — |
| `provider.base_url` | Base URL (for `http` type) | `https://localhost/artifacts` |
| `auth.type` | Authentication: `none`, `api_key`, `jwt` | `none` |
| `cache.max_size_mb` | Maximum cache size in megabytes | `500` |
| `telemetry.enabled` | Enable telemetry logging | `true` |
| `telemetry.remote_endpoint` | Optional remote telemetry URL | `null` |
| `offline_mode` | Use only cached artifacts | `false` |

See [`examples/launcher.json`](examples/launcher.json) for a complete example.

### Artifact Manifest

Tools are described via a JSON manifest. See [`examples/manifest.json`](examples/manifest.json) for the format.

---

## Local Storage Structure

All runtime data is stored under `%APPDATA%\AutomationLauncher`:

```
AutomationLauncher/
├── config/          # launcher.json settings
├── cache/           # Cached artifacts (by tool/version)
├── downloads/       # Temporary download files
├── logs/            # Telemetry and diagnostic logs
└── metadata/        # Artifact index, manifest cache
```

---

## Project Structure

```
automation-launcher/
├── Cargo.toml              # Workspace root
├── Cargo.lock
├── LICENSE
├── README.md               # This file — setup & usage
├── DEVELOPMENT.md          # What was built & development notes
├── .github/
│   └── workflows/
│       └── release.yml     # CI/CD pipeline
├── bootstrap/              # Bootstrap updater binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point, CLI, launch logic
│       ├── config.rs       # Bootstrap configuration
│       ├── error.rs        # Error types
│       ├── killswitch.rs   # Remote kill-switch mechanism
│       └── updater.rs      # Launcher update logic
├── launcher/               # Main launcher application
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point, app bootstrap
│       ├── cli.rs          # CLI argument parsing (clap)
│       ├── artifact.rs     # Artifact & manifest models
│       ├── config.rs       # Configuration system
│       ├── error.rs        # Error types
│       ├── gui.rs          # Graphical interface (feature-gated)
│       ├── commands/       # CLI command handlers
│       │   ├── mod.rs      # Shared helpers (fetch_manifest, format_bytes)
│       │   ├── cache.rs    # `cache` subcommand
│       │   ├── config.rs   # `config` subcommand
│       │   ├── download.rs # `download` subcommand
│       │   ├── fav.rs      # `fav` subcommand
│       │   ├── list.rs     # `list` subcommand
│       │   ├── run.rs      # `run` subcommand
│       │   └── search.rs   # `search` subcommand
│       ├── providers/      # Remote artifact providers
│       │   ├── mod.rs
│       │   ├── auth.rs     # Authentication (API Key, JWT, OAuth)
│       │   └── provider.rs # Provider trait, GitHub & HTTP impls
│       ├── services/       # Core service modules
│       │   ├── mod.rs
│       │   ├── cache.rs    # LRU cache management
│       │   ├── download.rs # Resumable download manager
│       │   ├── execution.rs# Process execution system
│       │   ├── extract.rs  # ZIP extraction
│       │   └── favorites.rs# Favorites management
│       └── telemetry/      # Observability
│           ├── mod.rs
│           ├── logging.rs  # Structured logging setup
│           └── telemetry.rs# Telemetry events & batching
├── docs/                   # Documentation
│   ├── architecture.md
│   ├── artifact_model.md
│   ├── authentication.md
│   ├── bootstrap_design.md
│   ├── caching.md
│   ├── download_system.md
│   ├── provider_system.md
│   ├── telemetry.md
│   └── internal/           # Planning & internal notes
│       ├── project-first-ideas-and-dev-plan.md
│       ├── answere-q-1.md
│       └── run-commands.md
└── examples/               # Sample configuration files
    ├── launcher.json       # Example launcher config
    └── manifest.json       # Example artifact manifest
```

---

## Building for Release

```bash
# Full release build
cargo build --release

# With GUI support
cargo build --release --features gui
```

Output binaries:
- `target/release/bootstrap.exe`
- `target/release/launcher.exe`

---

## CI/CD

The project includes a GitHub Actions workflow (`.github/workflows/release.yml`) that:

1. **On push to `main`/`develop` or PRs**: runs formatting check, clippy lints, tests, and builds release binaries
2. **On version tags (`v*`)**: creates a GitHub Release with binaries, zip package, and SHA256 checksums

To create a release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

---

## Documentation

Detailed design documentation is available in the [`docs/`](docs/) directory:

- [Architecture Overview](docs/architecture.md)
- [Artifact Model](docs/artifact_model.md)
- [Authentication System](docs/authentication.md)
- [Bootstrap Design](docs/bootstrap_design.md)
- [Cache Management](docs/caching.md)
- [Download System](docs/download_system.md)
- [Provider System](docs/provider_system.md)
- [Telemetry System](docs/telemetry.md)

---

## License

See [LICENSE](LICENSE) for details.
