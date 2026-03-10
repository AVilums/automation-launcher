# AI Development Instructions - Automation Launcher

This document provides context and guidelines for AI models (Copilot, Cursor, etc.) working on the **Automation Launcher** project.

## Project Overview

- **Name:** Automation Launcher
- **Purpose:** A Windows-first application for securely distributing, managing, and executing automation tools.
- **Architecture:** 
  - `launcher`: Main application (CLI + optional GUI).
  - `bootstrap`: A minimal updater that ensures the launcher is kept up-to-date.
  - **Workspace:** The project is a Rust workspace containing both crates.

## Technical Stack

- **Language:** Rust 2024 Edition.
- **Runtime:** `tokio` (asynchronous I/O).
- **CLI:** `clap` (derive-based command parsing).
- **GUI:** `egui` / `eframe` (feature-gated behind `gui`).
- **Networking:** `reqwest` for downloads and manifest fetching.
- **Serialization:** `serde` / `serde_json`.
- **Error Handling:** `thiserror` (structured errors) and `anyhow` (contextual errors in high-level commands).
- **Logging/Telemetry:** `tracing` + `tracing-subscriber`.

## Core Logic & Data Flows

1. **Artifacts:** Metadata about tools (name, version, checksums, launch configs).
2. **Providers:** Pluggable sources for manifests (GitHub Releases, HTTP).
3. **Execution:** Securely launching `.exe`, `.ps1`, and `.bat` files after checksum verification.
4. **Caching:** LRU-based caching in `%APPDATA%\AutomationLauncher\cache`.
5. **Bootstrap:** Checks for launcher updates before starting it.

## Coding Standards

### 1. Error Handling
- Use the `LauncherError` enum in `launcher/src/error.rs` for domain-specific errors.
- Use `map_err` to wrap low-level I/O or network errors into `LauncherError` variants.
- Avoid `unwrap()` and `expect()` in production code. Prefer returning `Result`.

### 2. Async/Await
- Use `tokio::main` for the entry points.
- Prefer non-blocking I/O via `tokio::fs` when working within the async context.

### 3. Logging & Telemetry
- Use `tracing` macros (`info!`, `warn!`, `error!`, `debug!`) instead of `println!`.
- Record business events via `TelemetryManager` for analytics.

### 4. GUI (egui)
- GUI code is located in `launcher/src/gui.rs` and is feature-gated: `#[cfg(feature = "gui")]`.
- Keep the GUI logic responsive; offload long-running tasks (like downloads) to the tokio runtime.

## Common Tasks for AI

- **Adding a Command:** Update `launcher/src/cli.rs` (Commands enum) and `launcher/src/commands/mod.rs`.
- **New Provider:** Implement the `ArtifactProvider` trait in `launcher/src/providers/`.
- **Telemetry Event:** Add a variant to `EventType` in `launcher/src/telemetry/mod.rs`.
- **Config Change:** Update `LauncherConfig` in `launcher/src/config.rs`.

## Security Considerations

- Always verify SHA256 checksums before executing any downloaded artifact.
- Validate paths to prevent directory traversal when managing the cache.
- Ensure sensitive information (like API keys) is handled securely in the config.

## Development Commands

- Build: `cargo build`
- Build with GUI: `cargo build --features gui`
- Test: `cargo test --all`
- Lint: `cargo clippy`
- Format: `cargo fmt`
