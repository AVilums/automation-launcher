### 1. Workspace Architecture

The platform evolves from the current two-executable monolith (`bootstrap/`, `launcher/`) into a modular Rust workspace with three executables and eleven shared crates.

```
automation-platform/
├── Cargo.toml                  # workspace root
├── bootstrap/                  # executable: bootstrap updater
├── launcher/                   # executable: attended GUI + CLI
├── runner/                     # executable: unattended agent
└── crates/
    ├── domain/                 # data models, validation, shared types
    ├── artifact/               # package format, manifest, checksums
    ├── provider/               # artifact source abstraction
    ├── runtime/                # runtime detection & validation
    ├── execution/              # execution pipeline (shared by launcher + runner)
    ├── storage/                # cache, LRU eviction, disk management
    ├── telemetry/              # structured event logging, tracing
    ├── auth/                   # authentication strategies
    ├── config/                 # configuration loading, layered config
    ├── scheduler/              # task scheduling, retry, timeout policies
    └── network/                # HTTP client, download, retry, streaming
```

**Current state:** `bootstrap/` and `launcher/` exist as workspace members. All launcher logic lives inside `launcher/src/` as modules. No shared crates or runner exist yet.

**Target state:** Extract shared modules from `launcher/` into workspace crates; add `runner/` executable.

---

### 2. Crate Responsibilities

#### Domain Crates (no side effects, no I/O)

| Crate | Responsibility |
|-------|---------------|
| `domain` | Core types: `AutomationId`, `Version`, `TaskId`, `MachineId`, `RunnerStatus`, enums for execution state, permission model, error types. Pure validation logic. |
| `artifact` | `Manifest` struct (parsed from `manifest.yaml`), package structure validation, checksum models (`SHA256`, `BLAKE3`), archive format detection, version resolution logic. |

#### Service Crates (implement functionality, depend on domain)

| Crate | Responsibility |
|-------|---------------|
| `provider` | `ArtifactProvider` trait + implementations: `GitHubReleasesProvider`, `HttpManifestProvider`, `ObjectStorageProvider`, `InternalApiProvider`. Auth-aware. |
| `runtime` | Detect and validate runtime requirements (Python, Node, .NET, native). Map manifest `runtime` field to local capabilities. |
| `execution` | The shared execution pipeline: discover → resolve → download → verify → extract → validate runtime → execute → capture logs → emit result. Used identically by launcher and runner. |
| `storage` | Local cache management, LRU eviction, disk usage tracking, artifact extraction target dirs, cleanup. Wraps current `services/cache.rs`. |
| `telemetry` | Structured telemetry: execution events, errors, durations, runner heartbeats. Pluggable sinks (file, HTTP, stdout). |
| `auth` | Authentication strategies: `NoAuth`, `ApiKey`, `JWT`, `OAuthDeviceFlow`. Token refresh, credential storage. Wraps current `providers/auth/`. |
| `config` | Layered configuration: defaults → file → env → CLI overrides. Typed config structs per component. |
| `scheduler` | Task queue, retry policies (exponential backoff, max attempts), execution timeouts, concurrency limits. Used by runner. |
| `network` | HTTP client wrapper, streaming downloads with progress, retry with backoff, checksum-on-stream. Wraps current `services/download.rs`. |

---

### 3. Dependency Boundaries

Strict layering is enforced — no upward or circular dependencies.

```
Executables (bootstrap, launcher, runner)
    ↓ depend on
Service Crates (provider, runtime, execution, storage, telemetry, auth, config, scheduler, network)
    ↓ depend on
Domain Crates (domain, artifact)
```

**Rules:**
- Domain crates depend only on `serde`, `thiserror`, and standard library.
- Service crates depend on domain crates + external libraries (reqwest, tokio, etc.).
- Executables depend on service crates and orchestrate them. They never contain business logic.
- `execution` depends on `artifact`, `provider`, `runtime`, `storage`, `network`, `telemetry`.
- `provider` depends on `artifact`, `auth`, `network`.
- `scheduler` depends on `domain` only.
- `runner` depends on `execution`, `scheduler`, `config`, `telemetry`, `network`.
- `launcher` depends on `execution`, `config`, `telemetry`, `storage`, `provider` + GUI framework.

**Enforcement:** Use `cargo deny` or workspace-level `[patch]` auditing. CI checks that domain crates have zero I/O dependencies.

---

### 4. Automation Package Format

#### Archive Format

- **Primary:** `.tar.zst` (Zstandard-compressed tar) — fast decompression, streaming-friendly.
- **Fallback:** `.zip` — broad tooling compatibility.

#### Package Structure

```
automation-package-<id>-<version>.tar.zst
└── automation-package/
    ├── manifest.yaml
    ├── automation/          # scripts or compiled binaries
    │   └── main.exe         # or main.py, main.ps1, etc.
    ├── assets/              # optional: config files, templates, data
    ├── runtime/             # optional: bundled runtime deps
    └── checksums/
        ├── SHA256SUMS       # per-file checksums
        └── SIGNATURE        # optional: Ed25519 detached signature
```

#### `manifest.yaml` Schema

```yaml
id: "free-resource"
version: "1.0.4"
name: "Free Resource Cleaner"
description: "Cleans Windows temporary directory to free up disk space."
entrypoint: "automation/free-resource.exe"

runtime:
  type: native              # native | python | node | powershell | dotnet
  version: null             # optional: ">=3.10" for python, etc.

inputs:
  - name: target_dir
    type: string
    required: false
    default: "%TEMP%"

execution:
  timeout_seconds: 300
  working_dir: "."          # relative to extraction root
  env:
    LOG_LEVEL: "info"
  elevated: false

permissions:
  filesystem: [read, write]
  network: false
  registry: false

telemetry:
  emit_start: true
  emit_completion: true
  emit_stdout: false
  custom_events: []

integrity:
  algorithm: sha256
  archive_checksum: "abc123..."  # checksum of the .tar.zst itself
  manifest_signature: null       # optional Ed25519 signature
```

**Migration note:** The current `manifest.json` format (seen in `free-resource`) should be supported during transition via a compatibility adapter in the `artifact` crate, converting JSON manifests to the canonical `manifest.yaml` model.

---

### 5. Execution Lifecycle

Both launcher and runner use the identical pipeline from the `execution` crate:

```
┌─────────────────┐
│ 1. Discovery    │  ← provider.list_artifacts() / task assignment
├─────────────────┤
│ 2. Resolution   │  ← version constraint → concrete version
├─────────────────┤
│ 3. Cache Check  │  ← storage.has(id, version)? → skip download
├─────────────────┤
│ 4. Download     │  ← network.download_streaming(url, progress)
├─────────────────┤
│ 5. Verification │  ← compare SHA256 of archive against manifest/provider
│                 │  ← optional signature verification (Ed25519)
├─────────────────┤
│ 6. Extraction   │  ← decompress .tar.zst / .zip into cache dir
├─────────────────┤
│ 7. File Verify  │  ← verify per-file checksums from checksums/SHA256SUMS
├─────────────────┤
│ 8. Runtime Val. │  ← runtime.validate(manifest.runtime) → error if missing
├─────────────────┤
│ 9. Execution    │  ← spawn process, apply timeout, capture stdout/stderr
├─────────────────┤
│ 10. Log Capture │  ← stream output to telemetry sink + local log
├─────────────────┤
│ 11. Result Emit │  ← ExecutionResult { status, exit_code, duration, logs }
└─────────────────┘
```

**Error handling:** Each stage returns `Result<T, ExecutionStageError>`. Failures at any stage halt the pipeline and emit a structured error event via telemetry. Retry is handled at the caller level (scheduler for runner, user-initiated for launcher).

---

### 6. Runner Lifecycle

```
┌──────────────────────────────────────────────────┐
│                  RUNNER AGENT                     │
│                                                   │
│  1. STARTUP                                       │
│     - Load config (config crate)                  │
│     - Initialize telemetry                        │
│     - Register with control plane (POST /register)│
│                                                   │
│  2. MAIN LOOP                                     │
│     ┌─────────────────────────────┐               │
│     │  Send heartbeat (interval)  │◄──────┐       │
│     │  Poll for tasks             │       │       │
│     │  If task available:         │       │       │
│     │    Execute via pipeline     │       │       │
│     │    Report result            │       │       │
│     │  Check for self-update      │       │       │
│     │  Sleep(poll_interval)       │───────┘       │
│     └─────────────────────────────┘               │
│                                                   │
│  3. SHUTDOWN                                      │
│     - Drain running tasks                         │
│     - Deregister / mark offline                   │
│     - Flush telemetry                             │
└──────────────────────────────────────────────────┘
```

**Concurrency model:** Tokio runtime with configurable `max_concurrent_tasks`. Each task runs in a spawned task with its own timeout via `tokio::time::timeout`.

**Retry policy** (from `scheduler` crate):
```
max_retries: 3
backoff: exponential (1s, 2s, 4s)
retry_on: [download_failure, timeout, transient_error]
no_retry_on: [checksum_mismatch, permission_denied, missing_runtime]
```

**Self-update:** Runner checks a designated update endpoint during each poll cycle. If a newer runner binary is available, it downloads, verifies, and triggers a graceful restart (or delegates to a service manager).

**Deployment:** Runner ships as a single binary installable as a Windows Service (`sc.exe create`) or systemd unit on Linux.

---

### 7. GUI Structure

Framework recommendation: **egui** (via `eframe`) — pure Rust, no web dependencies, feature-gated behind `--features gui`.

#### Tab Layout

```
┌──────────────────────────────────────────────────────────┐
│  [Automations]  [Favorites]  [Cache]  [Systems]  [⚙]    │
├──────────────────────────────────────────────────────────┤
│                                                          │
│              < tab content area >                        │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

#### Automations Tab

```
┌─────────────┬────────────────────────┬──────────────────┐
│ Navigation  │   Automation List      │  Detail Panel    │
│             │                        │                  │
│ ▸ Providers │  [🔍 Search...]        │  Name            │
│   ├ GitHub  │                        │  Description     │
│   └ HTTP    │  ┌──────────────────┐  │  Version: 1.0.4  │
│             │  │ Free Resource    │  │  Runtime: native │
│ ▸ Categories│  │ v1.0.4 · native  │  │  Timeout: 300s   │
│   ├ Cleanup │  │         [Run ▶]  │  │                  │
│   └ Reports │  ├──────────────────┤  │  Inputs:         │
│             │  │ Daily Report     │  │   target_dir     │
│ ★ Favorites │  │ v2.1.0 · python  │  │                  │
│             │  │         [Run ▶]  │  │  [Execute ▶]     │
└─────────────┴────────────────────────┴──────────────────┘
```

#### Cache Tab

```
┌──────────────────────────────────────────────────────────┐
│  Local Cache          Total: 245 MB    [Clean All]       │
├──────────────────────────────────────────────────────────┤
│  Artifact           Versions    Size      Actions        │
│  ─────────────────────────────────────────────────       │
│  free-resource      1.0.3,1.0.4  12 MB   [🗑 Remove]    │
│  daily-report       2.1.0        8 MB    [🗑 Remove]     │
│  data-sync          3.0.0,3.0.1  45 MB   [🗑 Remove]    │
└──────────────────────────────────────────────────────────┘
```

#### Systems Tab (Unattended)

```
┌──────────────────────────────────────────────────────────┐
│  Connected Runners                          [Refresh]    │
├──────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐               │
│  │ 🟢 PROD-SVR-01  │  │ 🟢 PROD-SVR-02  │               │
│  │ Windows 11      │  │ Ubuntu 22.04    │               │
│  │ Runner v0.3.0   │  │ Runner v0.3.0   │               │
│  │ Task: idle      │  │ Task: data-sync │               │
│  │ ♥ 2s ago        │  │ ♥ 5s ago        │               │
│  └─────────────────┘  └─────────────────┘               │
│  ┌─────────────────┐                                     │
│  │ 🔴 DEV-PC-03    │  Click a node to view:             │
│  │ Windows 10      │  • Recent tasks                    │
│  │ Runner v0.2.9   │  • Runtime capabilities            │
│  │ Last seen: 5m   │  • Logs                            │
│  └─────────────────┘                                     │
└──────────────────────────────────────────────────────────┘
```

#### Settings Tab

Sections: Authentication, Providers, Execution Defaults, Cache Limits, Telemetry, About.

---

### 8. Unattended System Visualization

The **Systems** tab in the launcher provides a dashboard for monitoring runner fleet status.

**Data flow:**
- Runners send heartbeats to a lightweight coordination API (or shared storage).
- Launcher polls the same API to retrieve runner status.
- No direct launcher→runner connection required.

**Node card fields:**
| Field | Source |
|-------|--------|
| Machine name | Runner registration payload |
| OS | `std::env::consts::OS` + version |
| Runner version | Compiled into runner binary |
| Status | Derived from heartbeat recency |
| Current task | Reported in heartbeat |
| Last heartbeat | Timestamp from API |

**Detail view** (on node click):
- **Recent tasks:** Table of last N executions with status, duration, timestamp.
- **Runtime capabilities:** Detected runtimes (Python 3.11, .NET 8, etc.).
- **Logs:** Streamed or paginated log viewer for the selected runner.

---

### 9. Artifact Verification Model

#### Three-Layer Verification

```
Layer 1: Transport Integrity
    └── HTTPS for all downloads (TLS verification)

Layer 2: Archive Integrity
    └── SHA256 checksum of the .tar.zst archive
    └── Checksum provided by provider metadata or manifest
    └── Verified BEFORE extraction

Layer 3: Content Integrity
    └── Per-file SHA256 checksums in checksums/SHA256SUMS
    └── Verified AFTER extraction
    └── Optional: Ed25519 signature over SHA256SUMS file
```

**Verification flow:**
1. Provider returns `(download_url, expected_sha256)`.
2. `network` crate downloads while computing SHA256 incrementally.
3. Compare computed hash against expected — reject on mismatch.
4. Extract archive.
5. Read `checksums/SHA256SUMS`, verify each file.
6. If `SIGNATURE` present and signing key configured, verify Ed25519 signature.

**Failure policy:** Any checksum mismatch is a hard failure. The artifact is deleted from cache. The event is logged to telemetry with full details. No retry (non-transient error).

**Best practice:** Providers should serve checksums out-of-band (e.g., in API response metadata) rather than only inside the archive, to prevent tampering of the archive itself.

---

### 10. Phased Development Roadmap

#### Phase 0 — Foundation Extraction (Current → +2 weeks)

**Goal:** Extract shared logic from `launcher/` into workspace crates without changing behavior.

| Task | Details |
|------|---------|
| Create `crates/domain` | Extract `AutomationId`, `Version`, error types from `artifact.rs`, `error.rs` |
| Create `crates/artifact` | Extract manifest parsing, checksum models from `artifact.rs` |
| Create `crates/config` | Extract from `config.rs`, make generic for all executables |
| Create `crates/network` | Extract from `services/download.rs` |
| Create `crates/auth` | Extract from `providers/auth/` |
| Create `crates/storage` | Extract from `services/cache.rs` |
| Create `crates/telemetry` | Extract from `telemetry/` |
| Migrate `manifest.json` → `manifest.yaml` | Add compatibility layer in `artifact` crate |
| CI | Add `cargo deny`, enforce dependency layering |

#### Phase 1 — Service Crate Assembly (+2–4 weeks)

**Goal:** Build higher-level service crates on top of domain crates.

| Task | Details |
|------|---------|
| Create `crates/provider` | Refactor `providers/` into trait + implementations using `auth` + `network` |
| Create `crates/runtime` | Runtime detection and validation logic |
| Create `crates/execution` | Assemble the full execution pipeline as a reusable service |
| Refactor `launcher/` | Thin executable that orchestrates crates, CLI via `clap` |
| Refactor `bootstrap/` | Use `config`, `network`, `telemetry` crates |
| Tests | Integration tests for execution pipeline with mock provider |

#### Phase 2 — Automation Package Format (+4–6 weeks)

**Goal:** Implement the `.tar.zst` package format with full verification.

| Task | Details |
|------|---------|
| Package builder tool | CLI tool or script to produce `.tar.zst` packages from source |
| `manifest.yaml` validation | JSON Schema or programmatic validation in `artifact` crate |
| Per-file checksum generation | `checksums/SHA256SUMS` generation in build tool |
| Ed25519 signing | Optional signing support in build tool + verification in `artifact` |
| Archive streaming extraction | `zstd` + `tar` crate integration in `storage` |
| Backward compat | Support `.zip` and legacy `manifest.json` |

#### Phase 3 — Runner Agent (+6–10 weeks)

**Goal:** Build the unattended runner executable.

| Task | Details |
|------|---------|
| Create `crates/scheduler` | Task queue, retry policies, concurrency limits |
| Create `runner/` executable | Registration, heartbeat, poll loop |
| Control plane API | Minimal HTTP API for task assignment + result collection (can be a separate service or embedded) |
| Concurrent execution | Tokio task spawning with configurable limits |
| Self-update mechanism | Download + verify + restart flow |
| Service installation | Windows Service wrapper, systemd unit generation |
| Tests | End-to-end: submit task → runner picks up → executes → reports |

#### Phase 4 — GUI Enhancement (+10–14 weeks)

**Goal:** Extend the launcher GUI with Systems tab and polish.

| Task | Details |
|------|---------|
| Systems tab | Node cards, status polling, detail view |
| Automation detail panel | Inputs form, execution progress, log viewer |
| Cache management UI | Disk usage visualization, selective cleanup |
| Settings UI | Full provider + auth + telemetry configuration |
| Notifications | Toast notifications for execution completion |
| Keyboard shortcuts | Power-user navigation |

#### Phase 5 — Hardening & Operations (+14–18 weeks)

**Goal:** Production readiness.

| Task | Details |
|------|---------|
| `crates/network` | Connection pooling, proxy support, bandwidth throttling |
| Telemetry sinks | HTTP sink for centralized logging, OpenTelemetry export |
| Audit logging | Who ran what, when, from where |
| Graceful degradation | Offline mode, stale cache usage, provider failover |
| Cross-platform | Linux + macOS support for launcher and runner |
| Documentation | Operator guide, package author guide, API docs |
| Performance | Benchmark execution pipeline, optimize cache lookups |
| Security audit | Dependency audit, path traversal prevention, privilege escalation review |

---

### Best Practices

- **Feature gates:** GUI code behind `#[cfg(feature = "gui")]` — runner and bootstrap never compile GUI dependencies.
- **Error types per crate:** Each crate defines its own error enum. Executables map them to user-facing messages.
- **No `unwrap()` in library crates.** All errors propagated via `Result`.
- **Structured logging everywhere:** Use `tracing` with span context. Every execution gets a unique `trace_id`.
- **Deterministic builds:** Pin all dependencies via `Cargo.lock`. Use `cargo-vet` or `cargo-deny` for supply chain auditing.
- **Integration test harness:** A `mock` provider and local HTTP server for testing the full pipeline without network.
- **Configuration precedence:** Defaults → config file → environment variables → CLI flags. Never hard-code paths.
- **Graceful shutdown:** All executables handle `SIGTERM`/`CTRL+C` — drain tasks, flush telemetry, exit cleanly.
- **Versioning:** Workspace-level version in root `Cargo.toml`. All crates share the same version. SemVer strictly.
- **Changelog:** Maintain `CHANGELOG.md` per phase. Automate with `git-cliff` or similar.
