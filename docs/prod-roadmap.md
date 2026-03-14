### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLIENT MACHINES                          │
│                                                                 │
│  ┌─────────────────┐          ┌──────────────────────────────┐  │
│  │  launcher.exe   │          │       runner.exe             │  │
│  │  (egui desktop) │          │  (Windows Service / daemon)  │  │
│  │                 │          │                              │  │
│  │  Browse tab     │          │  Scheduler loop              │  │
│  │  Systems tab    │          │  Execution pipeline          │  │
│  │  Cache tab      │          │  Heartbeat emitter           │  │
│  │  Settings tab   │          │  Log shipper                 │  │
│  └────────┬────────┘          └──────────────┬───────────────┘  │
└───────────┼───────────────────────────────────┼─────────────────┘
            │ HTTPS + API key                   │ HTTPS + API key
            ▼                                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                    HETZNER CAX11 (€3.29/mo)                     │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              rust_backend  (Actix-web 4)                 │   │
│  │                                                          │   │
│  │  /health              /runners                           │   │
│  │  /runners/heartbeat   /runners/:id/executions            │   │
│  │  /auth/token          /tenants/:id/runners               │   │
│  │                                                          │   │
│  │  SQLite (sqlx)  ←  single file, named Docker volume      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Caddy (reverse proxy, auto TLS via Let's Encrypt)              │
│  docker compose                                                 │
└─────────────────────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────┐
│              GITHUB (free CDN for automation packages)          │
│                                                                 │
│  registry.json  ←  provider crate fetches this                  │
│  temp-cleaner-1.0.0.tar.zst                                     │
│  disk-report-1.0.0.tar.zst                                      │
└─────────────────────────────────────────────────────────────────┘
```

---

### UML: Core Domain Model

```
┌──────────────┐       ┌──────────────────┐       ┌─────────────────┐
│   Tenant     │1    * │     Runner        │1    * │  ScheduledTask  │
│──────────────│───────│──────────────────│───────│─────────────────│
│ id: UUID     │       │ id: UUID         │       │ id: UUID        │
│ name: String │       │ tenant_id: UUID  │       │ runner_id: UUID │
│ api_key_hash │       │ hostname: String │       │ tool_name       │
│ created_at   │       │ status: enum     │       │ schedule: enum  │
└──────────────┘       │ last_seen: UTC   │       │ enabled: bool   │
                       │ version: String  │       │ last_run: UTC   │
                       └────────┬─────────┘       └────────┬────────┘
                                │1                         │
                                │*                         │*
                       ┌────────▼─────────┐      ┌────────▼────────┐
                       │  ExecutionRecord  │      │  Automation     │
                       │──────────────────│      │─────────────────│
                       │ id: UUID         │      │ id: String      │
                       │ runner_id: UUID  │      │ version: String │
                       │ tool_name        │      │ name: String    │
                       │ started_at: UTC  │      │ description     │
                       │ finished_at: UTC │      │ runtime: enum   │
                       │ exit_code: i32   │      │ download_url    │
                       │ summary: String  │      │ checksum_sha256 │
                       │ log_tail: String │      └─────────────────┘
                       └──────────────────┘

RunnerStatus: Online | Offline | Stale | Error
Schedule: Once(UTC) | Interval(secs) | Daily{hour, minute} | Cron(String)
Runtime: Native | Python | Node | PowerShell | DotNet
```

---

### UML: Execution Sequence (Attended — Launcher)

```
User          Launcher GUI       execution crate     provider crate     Backend API
 │                │                    │                   │                │
 │  click Run     │                    │                   │                │
 │───────────────►│                    │                   │                │
 │                │ run_automation()   │                   │                │
 │                │───────────────────►│                   │                │
 │                │                   │ fetch_manifest()   │                │
 │                │                   │───────────────────►│                │
 │                │                   │◄───────────────────│                │
 │                │                   │ cache_check()      │                │
 │                │                   │──────┐             │                │
 │                │                   │◄─────┘             │                │
 │                │                   │ download+verify()  │                │
 │                │                   │───────────────────►│                │
 │                │                   │◄───────────────────│                │
 │                │                   │ extract+checksum() │                │
 │                │                   │──────┐             │                │
 │                │                   │◄─────┘             │                │
 │                │                   │ spawn process      │                │
 │                │                   │──────┐             │                │
 │  stream logs   │◄───────────────────│     │             │                │
 │◄───────────────│                   │     │             │                │
 │                │                   │◄────┘ exit        │                │
 │                │                   │ POST /executions   │                │
 │                │                   │────────────────────────────────────►│
 │                │                   │◄────────────────────────────────────│
 │  show result   │◄───────────────────│                   │                │
 │◄───────────────│                   │                   │                │
```

---

### UML: Execution Sequence (Unattended — Runner Daemon)

```
Runner Daemon      Scheduler       execution crate      Backend API
     │                 │                 │                   │
     │  tick (60s)     │                 │                   │
     │────────────────►│                 │                   │
     │                 │ due_tasks()     │                   │
     │                 │──────┐          │                   │
     │                 │◄─────┘          │                   │
     │  run_task(t)    │                 │                   │
     │────────────────────────────────►  │                   │
     │                                  │ [same pipeline]   │
     │                                  │ spawn+capture     │
     │                                  │──────┐            │
     │                                  │◄─────┘            │
     │                                  │ POST /executions  │
     │                                  │──────────────────►│
     │                                  │◄──────────────────│
     │  POST /heartbeat                 │                   │
     │──────────────────────────────────────────────────────►│
     │◄──────────────────────────────────────────────────────│
     │  sleep(60s)     │                 │                   │
```

---

### Phase 1: Backend Rebuild (rust_backend)

**Goal:** Replace pizza CRUD with the platform domain. TDD throughout — write the test, then the handler.

#### Database Schema (SQLite via sqlx)

Four tables:
- `tenants` — id, name, api_key_hash (argon2), created_at
- `runners` — id, tenant_id, hostname, machine_id, version, status, last_seen
- `executions` — id, runner_id, tool_name, tool_version, started_at, finished_at, exit_code, summary, log_tail
- `scheduled_tasks` — id, runner_id, tool_name, schedule_json, enabled, last_run

Migrations via `sqlx migrate` — versioned SQL files in `migrations/`. Never alter tables manually.

#### API Endpoints

```
GET  /health                              — liveness probe, no auth
POST /runners/heartbeat                   — runner posts status; upserts runner row
GET  /runners                             — launcher fetches all runners for tenant
GET  /runners/:id                         — single runner detail + last 10 executions
POST /runners/:id/executions              — runner posts execution result
GET  /runners/:id/executions?limit=50     — launcher fetches history
GET  /runners/:id/tasks                   — launcher fetches scheduled tasks
PUT  /runners/:id/tasks/:task_id          — launcher updates a scheduled task
```

#### Security

- **API key auth:** Every request (except `/health`) requires `X-Api-Key` header. Key is hashed with argon2 at registration, compared on every request. Never store plaintext.
- **Tenant isolation:** Every DB query includes `tenant_id` from the resolved API key. A runner from tenant A cannot read tenant B's data — enforced at query level, not application logic level.
- **Rate limiting:** `actix-governor` middleware — 60 req/min per IP for heartbeat, 600 req/min for reads.
- **Input validation:** `validator` crate on all request bodies. Reject oversized payloads (max 64KB for log tails).
- **No secrets in logs:** Middleware strips `X-Api-Key` from access logs before writing.
- **HTTPS only:** Caddy handles TLS termination. Backend binds to `127.0.0.1:8080` — never exposed directly.

#### TDD Approach

Each handler gets:
1. Unit test: pure function logic (schedule parsing, status transitions)
2. Integration test: `actix_web::test` with in-memory SQLite — test the full HTTP stack
3. Auth test: verify 401 on missing key, 403 on wrong tenant, 200 on valid key

Test structure:
```
tests/
  integration/
    health.rs
    runners.rs
    executions.rs
    auth.rs
  fixtures/
    seed.sql       — test tenant + runner rows
```

---

### Phase 2: Runner → Backend Integration

**Goal:** Runner daemon posts heartbeat and execution results to the backend instead of writing local files.

#### Changes to runner

- Add `backend_url: Option<String>` and `api_key: Option<String>` to `LauncherConfig`
- In daemon loop: after each task completes, `POST /runners/:id/executions`
- Every tick: `POST /runners/heartbeat` with current status, version, hostname
- If backend is unreachable: log warning, continue — never crash the daemon over telemetry failure
- Runner registers itself on first heartbeat (upsert semantics on backend)

#### Runner as Windows Service

- Use `windows-service` crate to install/uninstall/start/stop
- `runner install --backend-url https://api.yourdomain.com --api-key <key>` — writes config, registers service
- `runner uninstall` — removes service
- Service name: `OrgOnAutoRunner`, display name: `Org On Auto Runner`
- Runs as `LocalService` account — no admin rights needed for most automations
- Logs to Windows Event Log via `tracing` + `tracing-eventlog` sink

#### TDD Approach

- Mock backend: `wiremock` crate — spin up a local HTTP mock, assert runner posts correct JSON
- Test heartbeat retry: mock returns 503 twice, then 200 — assert runner retries with backoff
- Test execution posting: run a real `echo hello` automation, assert execution record posted

---

### Phase 3: Launcher → Backend Integration

**Goal:** Systems tab shows live data from backend, not demo data.

#### Changes to launcher

- `systems_data.rs`: replace hardcoded demo runners with `GET /runners` call to backend
- Poll every 30 seconds (configurable), show last-seen timestamp with staleness indicator
- Settings tab: add `Backend URL` and `API Key` fields — saved to config file
- Execution history panel: click a runner → fetch `GET /runners/:id/executions` → show table
- Schedule management: click a task → edit schedule → `PUT /runners/:id/tasks/:task_id`

#### Security in launcher

- API key stored in OS credential store: `keyring` crate (Windows Credential Manager, macOS Keychain, libsecret on Linux) — never in plaintext config file
- Config file stores only the backend URL; key is fetched from keyring at runtime
- If keyring unavailable: fall back to env var `ORG_ON_AUTO_API_KEY`, warn user

---

### Phase 4: Automations (Two Production-Ready Examples)

#### `temp-cleaner` (Unattended)

Rust binary. Arguments: `--dry-run` (default: false), `--target <dir>` (default: `%TEMP%`), `--min-age-hours <n>` (default: 0).

Output format (one line per event, machine-parseable):
```
[INFO] scan_start target=C:\Users\...\AppData\Local\Temp
[INFO] file_deleted path=... size_bytes=4096
[WARN] file_skipped path=... reason=in_use
[INFO] scan_complete files_deleted=1847 bytes_freed=149123072 errors=12 duration_ms=3241
```

Exit codes: 0 = success, 1 = partial (some files skipped), 2 = fatal error.

Manifest inputs: `dry_run: bool`, `target_dir: string`, `min_age_hours: u32`.

TDD: unit tests for the file walker, deletion logic, size accumulator. Integration test: create temp dir with known files, run cleaner, assert files gone and output correct.

#### `disk-report` (Attended)

Rust binary. Arguments: `--target <dir>` (default: user home), `--top <n>` (default: 20), `--min-size-mb <n>` (default: 10).

Output: clean aligned table streamed to stdout as each directory is sized:
```
Scanning C:\Users\artur... (this may take a moment)

Rank  Size       Path
────  ─────────  ────────────────────────────────────────
   1  42.3 GB    C:\Users\artur\Downloads
   2  18.7 GB    C:\Users\artur\Videos
   ...
  20   1.2 GB    C:\Users\artur\AppData\Local\npm-cache

Total scanned: 847 directories  Duration: 4.2s
```

Exit codes: 0 = success, 1 = partial (some dirs inaccessible), 2 = fatal.

TDD: unit tests for size formatting, ranking logic. Integration test: create known directory tree, assert correct top-N output.

#### Package structure for both

```
automations/
  temp-cleaner/
    src/main.rs
    Cargo.toml
    manifest.yaml
    README.md
    tests/
  disk-report/
    src/main.rs
    Cargo.toml
    manifest.yaml
    README.md
    tests/
scripts/
  package.ps1     — build → structure → checksum → tar.zst → optional gh release upload
```

---

### Phase 5: Infrastructure & Docker

#### `docker-compose.yml` (production)

Two services: `backend` and `caddy`. No database container — SQLite in a named volume.

```
services:
  backend:   Actix binary, binds 127.0.0.1:8080, volume /data for SQLite
  caddy:     Reverse proxy, auto TLS, forwards api.yourdomain.com → backend:8080
```

#### Caddy config

- `api.yourdomain.com` → `http://backend:8080`
- Auto HTTPS via Let's Encrypt (Caddy handles this natively)
- HSTS header, no HTTP fallback

#### Hetzner setup steps

1. Provision CAX11 (ARM64, Ubuntu 24.04)
2. `apt install docker.io docker-compose-plugin`
3. Add non-root deploy user, SSH key only, disable password auth
4. UFW: allow 22, 80, 443 only
5. `git clone` repo, `docker compose up -d`
6. Point DNS A record at server IP
7. Caddy auto-provisions TLS on first request

#### Backup

- Daily `sqlite3 /data/backend.db .dump > backup-$(date +%Y%m%d).sql`
- Upload to Hetzner Object Storage (S3-compatible, €0.023/GB/month) via `rclone`
- Keep 30 days of backups
- Cron job on the server, not inside Docker

#### SQLite production hardening

- `PRAGMA journal_mode=WAL` — enables concurrent reads during writes
- `PRAGMA synchronous=NORMAL` — safe for WAL mode, faster than FULL
- `PRAGMA foreign_keys=ON` — enforce referential integrity
- `PRAGMA busy_timeout=5000` — 5s timeout instead of immediate SQLITE_BUSY
- Connection pool: max 1 writer, N readers (sqlx handles this)

---

### Phase 6: Cloud GUI Automation (Windows)

For attended automations that need a real Windows desktop (clicking, screen reading, etc.):

#### Option A: Hetzner Windows VPS (cheapest managed)

Hetzner doesn't offer Windows. Use **Contabo VPS S** with Windows Server 2022 (~€8/month including license). RDP in, install runner as a service, configure it to run attended automations on a schedule with a logged-in session.

#### Option B: GitHub Actions Windows Runner (free for public, $0.008/min for private)

For automations that run < 6 hours/month: trigger via GitHub Actions `workflow_dispatch` on a `windows-latest` runner. The runner posts results back to your backend. Cost: essentially free for demos.

#### Option C: Oracle Free Tier ARM + Wine (for Linux-compatible automations)

For AutoIt or simple GUI scripts that work under Wine. Free forever, 4 vCPU / 24GB RAM ARM instance.

**Recommended for MVP1:** Use your local Windows machine as the "cloud Windows runner" — install the runner as a Windows Service, point it at your Hetzner backend. This gives you a real attended + unattended demo without any Windows cloud cost. Add a Contabo Windows VPS when you have a paying client who needs it.

---

### Security Checklist (Production)

| Layer | Control |
|-------|---------|
| Transport | TLS 1.2+ enforced by Caddy, HSTS |
| Authentication | API key hashed with argon2id (cost factor 3, 64MB memory) |
| Authorization | Tenant isolation enforced at every SQL query |
| Package integrity | SHA256 archive checksum + per-file checksums verified before execution |
| Package signing | Ed25519 signature verification (optional for MVP1, required for MVP2) |
| Rate limiting | actix-governor, per-IP |
| Input validation | validator crate on all request bodies, max payload 64KB |
| Secrets | API keys in OS keyring (launcher), never in config files or logs |
| DB | WAL mode, foreign keys, parameterized queries only (sqlx compile-time checked) |
| Server | UFW firewall, SSH key only, non-root deploy user, fail2ban |
| Dependencies | cargo-deny for license + vulnerability auditing in CI |
| Logs | No secrets in logs, structured JSON via tracing-subscriber |

---

### TDD Strategy

**Rule:** No handler, no crate function, no automation logic ships without a test written first.

| Component | Test type | Tool |
|-----------|-----------|------|
| Backend handlers | Integration (full HTTP stack) | `actix_web::test` + in-memory SQLite |
| Auth middleware | Integration | `actix_web::test` |
| Tenant isolation | Integration | Two tenants, assert cross-access returns 403 |
| Runner heartbeat posting | Integration | `wiremock` mock server |
| Execution pipeline | Integration | Real subprocess (`echo`), assert result |
| Automation logic | Unit | Standard `#[test]` |
| Scheduler due-task logic | Unit | Mocked time via `chrono` |
| Config loading | Unit | Temp files, env vars |
| Package checksum | Unit | Known file + known hash |

CI runs `cargo test --workspace` on every push. No `#[ignore]` without a linked issue.

---

### Ordered Work Items

**Week 1 — Backend**
1. Replace SurrealDB with sqlx + SQLite; write migrations
2. Implement `POST /runners/heartbeat` with tenant auth — test first
3. Implement `GET /runners`, `GET /runners/:id` — test first
4. Implement `POST /runners/:id/executions`, `GET /runners/:id/executions` — test first
5. Add API key middleware with argon2 verification — test auth + tenant isolation
6. Write `Dockerfile` + `docker-compose.yml` + Caddy config

**Week 2 — Runner**
7. Add backend URL + API key to config; post heartbeat in daemon loop
8. Post execution result after each task completes
9. Implement `runner install` / `runner uninstall` as Windows Service
10. Test with wiremock: heartbeat retry, execution posting

**Week 3 — Launcher**
11. Wire Settings tab: backend URL + API key (stored in keyring)
12. Wire Systems tab: `GET /runners` replaces demo data
13. Add execution history panel per runner
14. Add schedule edit UI → `PUT /runners/:id/tasks/:task_id`

**Week 4 — Automations + Packaging**
15. Build `temp-cleaner` in Rust with TDD
16. Build `disk-report` in Rust with TDD
17. Write `scripts/package.ps1`
18. Create `manifest.yaml` for both, package as `.tar.zst`
19. Publish to GitHub Releases, create `registry.json`

**Week 5 — Deploy + Demo**
20. Provision Hetzner CAX11, deploy via docker compose
21. Install runner as Windows Service on local machine, point at Hetzner backend
22. End-to-end smoke test: browse → download → run → heartbeat → history
23. Write `SETUP.md` — client onboarding doc
24. Write `SECURITY.md` — what's protected and how
