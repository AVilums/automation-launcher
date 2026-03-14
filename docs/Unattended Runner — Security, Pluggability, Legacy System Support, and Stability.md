### Security

#### How the runner is secured

**1. Package integrity (already designed in)**

Every automation package goes through three-layer verification before execution:
- **TLS** — all downloads over HTTPS, no plain HTTP
- **Archive checksum** — SHA256 of the `.tar.zst` verified before extraction
- **Per-file checksums** — `checksums/SHA256SUMS` verified after extraction
- **Optional Ed25519 signature** — you sign packages with your private key; runner verifies with your public key before running anything

This means a runner will **refuse to execute a tampered or unknown package** — even if someone intercepts the download.

**2. Runner identity and authentication**

When a runner registers with the control plane (or writes its heartbeat file), it should present a **machine token** — a pre-shared secret or short-lived JWT issued when you onboard the machine. The control plane rejects heartbeats and task polls from unknown tokens.

```toml
# runner config
[auth]
machine_token = "eyJ..."   # issued once during onboarding
control_plane_url = "https://your-api/runner"
```

**3. Execution isolation**

The runner spawns automations as **child processes** — not in-process. This means:
- A crashed automation doesn't crash the runner
- You can apply OS-level sandboxing (Windows Job Objects, Linux namespaces/cgroups) to the child process
- The manifest declares required permissions (`filesystem`, `network`, `registry`) — the runner can enforce these by refusing to run packages that claim more than the runner's policy allows

**4. Principle of least privilege**

The runner itself should run as a **low-privilege service account** (not SYSTEM on Windows, not root on Linux). Automations that need elevation must explicitly declare `elevated: true` in the manifest, and the runner can require a separate approval step before running them.

**5. Audit trail**

Every execution is logged with: who triggered it, what package + version, what machine, start/end time, exit code, stdout hash. This is your audit log — essential for client trust.

---

### Dynamic / Pluggable

#### Runner capabilities are declared, not hardcoded

Each runner reports its **capabilities** in its registration/heartbeat:

```json
{
  "machine_id": "PROD-SVR-01",
  "capabilities": ["native", "powershell", "python3.11", "dotnet8", "nodejs20"],
  "tags": ["windows", "production", "has-display"],
  "os": "Windows Server 2022"
}
```

The launcher (or control plane) uses these capabilities to **route tasks to compatible runners**. A Python automation only gets dispatched to runners that report `python3.x`.

#### Adding a new runner = zero config on the control plane

1. Install `runner.exe` on the new machine
2. Give it a config file with the control plane URL and a machine token
3. Start it — it registers itself, starts sending heartbeats, appears in the Systems tab

No manual registration, no firewall rules to open (runner polls outbound), no restart of anything else.

#### Pluggable automation sources (providers)

The `provider` trait abstraction means you can add new automation sources without changing the runner:
- GitHub Releases (current)
- Internal HTTP server
- S3/object storage
- Local directory (for air-gapped environments)

The runner just knows "fetch artifact from provider" — it doesn't care where the provider is.

#### Pluggable execution runtimes

The `runtime` crate detects what's available on the machine. You can add new runtime types (e.g., `wasm`, `container`) by implementing the runtime detection + execution interface. Existing automations are unaffected.

---

### Legacy System Support (RPA-style, like UiPath/Blue Prism)

This is the most nuanced area. Here's the honest picture:

#### What the runner can do natively

| Scenario | Supported? | How |
|----------|-----------|-----|
| Run a PowerShell script that automates a legacy app via COM | ✅ | `runtime: powershell` in manifest |
| Run a Python script using `pyautogui` / `pywinauto` for UI automation | ✅ | `runtime: python`, bundle deps |
| Run a .NET executable that uses UI Automation APIs | ✅ | `runtime: dotnet` |
| Run a compiled Rust binary that calls Win32 APIs | ✅ | `runtime: native` |
| Automate a web-based legacy system via headless browser | ✅ | Bundle Playwright/Selenium in the package |
| Automate a thick-client Windows app (click buttons, fill forms) | ✅ with caveats | Needs a **desktop session** (see below) |

#### The desktop session requirement

GUI automation (clicking, typing, screen reading) requires the runner to have an **active desktop session**. This is the same constraint UiPath/Blue Prism have — they call it "attended" vs "unattended" execution.

For unattended GUI automation on Windows:
- Run the runner as a **Windows Service with "Allow service to interact with desktop"** enabled (works for simple cases)
- Or use a **dedicated user session** — log in as a service account, keep the session active, run the runner in that session (this is how UiPath unattended robots work)
- Or use **virtual display** (e.g., a headless RDP session) — the runner has a display but no physical monitor needed

For Linux legacy systems (terminal-based, SSH, etc.) — no desktop needed, the runner handles these natively.

#### What you'd build for legacy RPA use cases

```
automation-package/
├── manifest.yaml          # runtime: python, elevated: false
└── automation/
    ├── main.py            # orchestration script
    └── requirements/      # bundled: pywinauto, pygetwindow, etc.
```

The runner extracts the package, validates the runtime, spawns `python main.py` — the Python script does the UI automation. This is functionally equivalent to a UiPath workflow, just without the visual designer.

**Key advantage over UiPath/Blue Prism:** Your packages are plain executables. No proprietary runtime license, no per-robot seat fee, no vendor lock-in.

---

### Stability

#### What makes the runner stable

**1. Process isolation**

Automations run as child processes. If an automation hangs, crashes, or consumes too much memory — the runner kills it (via `timeout_seconds` from the manifest) and reports the failure. The runner itself keeps running.

**2. Retry with backoff (from `scheduler` crate)**

```
max_retries: 3
backoff: exponential (1s → 2s → 4s)
retry_on: [download_failure, timeout, transient_error]
no_retry_on: [checksum_mismatch, permission_denied, missing_runtime]
```

Transient failures (network blip, slow download) are retried automatically. Permanent failures (bad package, wrong permissions) are not retried — they surface immediately.

**3. Heartbeat monitoring**

The launcher's Systems tab marks a runner as offline if its heartbeat is >30s stale. This gives you immediate visibility when a runner machine goes down, crashes, or loses network. You can set up alerts on this.

**4. Graceful shutdown**

On `SIGTERM` / `CTRL+C`, the runner:
1. Stops accepting new tasks
2. Waits for in-progress tasks to complete (or times out)
3. Marks itself offline in the control plane
4. Flushes telemetry
5. Exits cleanly

This means service restarts (for updates, reboots) don't leave orphaned processes or corrupt state.

**5. Self-update**

The runner checks for a newer version of itself on each poll cycle. If found, it downloads, verifies (same checksum model as automations), and triggers a graceful restart. No manual intervention needed to keep runners current.

**6. Idempotent task execution**

Tasks should be designed to be safe to re-run (idempotent). The runner tracks which tasks have been executed (with results) so it doesn't accidentally re-run a completed task after a restart.

**7. Concurrency limits**

```toml
[runner]
max_concurrent_tasks = 2   # never run more than 2 automations simultaneously
```

This prevents a flood of queued tasks from overwhelming the machine.

---

### Summary Table

| Concern | Mechanism |
|---------|-----------|
| **Secure delivery** | TLS + SHA256 + Ed25519 signatures |
| **Secure execution** | Least-privilege service account + process isolation + permission manifest |
| **Secure identity** | Machine tokens, runner registration |
| **Pluggable runners** | Self-registering, capability-based routing |
| **Pluggable sources** | `ArtifactProvider` trait — swap backends without changing runner |
| **Legacy GUI automation** | Python/PowerShell/.NET packages with UI automation libs, desktop session required |
| **Stability** | Process isolation, retry/backoff, heartbeat monitoring, graceful shutdown, self-update |
| **Auditability** | Structured telemetry on every execution event |

The architecture is deliberately similar to how enterprise RPA platforms work internally — but without the proprietary runtime, the visual designer lock-in, or the per-seat licensing. Your differentiator is that automations are **plain executables in standard packages**, runnable by anyone with the runner binary.
