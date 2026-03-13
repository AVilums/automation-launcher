# Junie Instructions — Automation Runtime Platform

## Project Identity

- **Name:** Automation Runtime Platform
- **Language:** Rust (2024 Edition)
- **Workspace:** Cargo workspace with executables (`bootstrap/`, `launcher/`, future `runner/`) and shared crates under `crates/`
- **Platform:** Windows-first, cross-platform later
- **Date:** March 2026

---

## Architecture Rules

### Dependency Layering (strict, never violate)

```
executables → service crates → domain crates
```

- **Domain crates** (`domain`, `artifact`): Pure data models, validation, no I/O, no async. Only depend on `serde`, `thiserror`, std.
- **Service crates** (`provider`, `runtime`, `execution`, `storage`, `telemetry`, `auth`, `config`, `scheduler`, `network`): Implement functionality. Depend on domain crates + external libs.
- **Executables** (`bootstrap`, `launcher`, `runner`): Orchestrate services. No business logic lives here.

Never introduce circular or upward dependencies. If you feel the need, you are missing an abstraction — create a new domain type or trait.

### Modularity (medium-high to high)

- Every distinct responsibility gets its own crate or module. When in doubt, split.
- Crates communicate through traits and domain types, not concrete implementations.
- Feature-gate optional capabilities (`gui`, `telemetry-http`, `provider-github`). Don't compile what you don't need.
- Keep crate public APIs minimal. Expose only what consumers need. Use `pub(crate)` liberally.

### Keep It Light

- Especially in early phases: prefer simple, working solutions over elaborate abstractions.
- Don't build infrastructure you don't need yet. A trait with one implementation is fine — don't add a registry, factory, or plugin system until there are three implementations.
- Flat module structures are preferred over deep nesting. A file with 200 lines is better than 5 files with 40 lines each if the concept is cohesive.

---

## Test-Driven Development

### The Core Rule

**Write tests first, or alongside, every change. Never after.**

- Before implementing a function, write at least one test that describes the expected behavior.
- A PR/change without tests for new logic is incomplete.

### Test Strategy

| Layer | Test Type | Approach |
|-------|-----------|----------|
| Domain crates | Unit tests | Pure functions, no mocks needed. Test validation, parsing, edge cases. |
| Service crates | Unit + integration | Use trait mocks/fakes for dependencies. Test happy path + error paths. |
| Executables | Integration / E2E | Test CLI commands with real crate wiring but mock providers/network. |

### Test Guidelines

- **No added complexity for tests.** Tests should be simple and readable. If a test needs 50 lines of setup, the code under test has too many dependencies — refactor.
- Use `#[cfg(test)] mod tests` in the same file for unit tests.
- Use `tests/` directory at crate root for integration tests.
- Prefer hand-written fakes over mocking frameworks. A `struct FakeProvider` implementing the trait is clearer than macro-generated mocks.
- Name tests descriptively: `fn download_fails_on_checksum_mismatch()` not `fn test_download_3()`.
- Test error paths. Every `Result::Err` variant should have at least one test that triggers it.
- Keep tests fast. No real network calls, no real filesystem writes to system dirs. Use `tempdir` for filesystem tests.

### What NOT to Test

- Don't test private implementation details that may change.
- Don't test third-party library behavior.
- Don't write tests that just assert the mock was called — test observable outcomes.

---

## Coding Standards

### Error Handling

- Each crate defines its own error enum via `thiserror`.
- No `unwrap()` or `expect()` in library crates. Ever.
- Executables may use `anyhow` for top-level error context.
- Map errors at crate boundaries: callers should not see internal error types.

### Async

- Use `tokio` for async runtime. Entry points use `#[tokio::main]`.
- Prefer `tokio::fs` over `std::fs` in async contexts.
- Don't make things async unless they do I/O. Domain logic stays synchronous.

### Logging

- Use `tracing` (`info!`, `warn!`, `error!`, `debug!`, `trace!`). Never `println!` in library code.
- Add span context for operations: `#[instrument]` on public service functions.

### Code Style

- `cargo fmt` before every commit. No exceptions.
- `cargo clippy -- -D warnings` must pass.
- Prefer explicit types on public API boundaries. Infer types internally.
- Document all public items with `///` doc comments. Internal items get comments only when non-obvious.

---

## Security

- Verify SHA256 checksums before executing any downloaded artifact. This is non-negotiable.
- Validate all file paths against directory traversal.
- Never log secrets, API keys, or tokens — even at `debug!` level.
- Use `secrecy` crate for sensitive values in memory when handling auth tokens.

---

## Development Workflow

### Commands

```
cargo build                    # build all
cargo build --features gui     # build with GUI
cargo test --workspace         # run all tests
cargo clippy --workspace       # lint
cargo fmt --all                # format
```

### Before Submitting Any Change

1. `cargo fmt --all`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace`
4. All three must pass with zero warnings.

### Commit Discipline

- Small, focused commits. One logical change per commit.
- Commit message format: `crate: short description` (e.g., `artifact: add manifest.yaml parsing`)

---

## Current State & Phasing

The project is in early development. `bootstrap/` and `launcher/` exist as workspace members. Shared crates under `crates/` are being extracted from `launcher/`.

### Phase Priorities (light approach)

**Phase 0 — Extract domain and artifact crates first.** Move types and validation out of `launcher/src/`. Write unit tests for every extracted type. Keep it simple — just move code, add tests, verify nothing breaks.

**Phase 1 — Extract service crates one at a time.** Each extraction: move code → add trait boundary → write tests with fakes → update launcher to use the crate. Don't extract everything at once.

**Phase 2+ — Build new features (runner, package format, GUI tabs) on top of the clean crate structure.** By this point, every new feature starts with a test.

### What "Light" Means

- Don't over-engineer early crates. A `config` crate that reads a TOML file and returns a struct is enough. No hot-reloading, no file watchers, no layered config system — until needed.
- A `provider` trait with one GitHub implementation is fine. Add the abstraction, skip the plugin registry.
- Prefer working software with tests over perfect architecture without tests.

---

## Common Tasks

| Task | Where |
|------|-------|
| Add a CLI command | `launcher/src/cli.rs` (enum) + `launcher/src/commands/` (handler) |
| Add a provider | Implement `ArtifactProvider` trait in `crates/provider/src/` |
| Add a config field | Update struct in `crates/config/src/` (or `launcher/src/config.rs` pre-extraction) |
| Add a telemetry event | Add variant to `EventType` in telemetry crate/module |
| Add a domain type | `crates/domain/src/` — pure struct, derive Serialize/Deserialize, add validation, add tests |
```
