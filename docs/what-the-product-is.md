### What the Product Actually Is

You're building an **automation delivery and execution platform**. Think of it as the missing layer between "I have a script" and "it runs reliably, on the right machine, at the right time, and I can see what happened."

The two-product split that makes sense:

| Product | What it is | Who it's for |
|---------|-----------|--------------|
| **Automation Launcher** (GUI) | Desktop app — browse, run, and manage automations attended | Individual power users, developers, IT staff |
| **Automation Runner** (daemon) | Headless agent — runs automations unattended on a schedule or on demand | Sysadmins, DevOps, small IT teams |

Together they form a lightweight **RPA/automation management platform** — but without the enterprise bloat and $50k/year price tag of UiPath or Automation Anywhere.

---

### Who Would Pay for This

**Primary target: small IT teams and solo developers** who:
- Have repetitive tasks they've half-automated with scripts
- Don't want to manage a full CI/CD pipeline just to run a cleanup job
- Want visibility into "did it run, did it work, what did it do"

**Concrete client profiles:**
- Freelance developer managing 5–10 client servers — needs to run maintenance scripts reliably
- Small agency with a sysadmin — wants scheduled tasks with a dashboard, not just Windows Task Scheduler
- Solo founder / indie hacker — wants to automate their own ops without learning Airflow

---

### How to Present It to Clients

Frame it as **"your automations, packaged and managed"** — not as a tool they configure, but as a service you deliver:

> *"I build the automation, package it, and deploy it to your machine. You open the launcher, click Run, and see the results. Or it runs automatically overnight and you check the log in the morning."*

The key differentiator vs. "just send me a script":
- **Versioned packages** — you push an update, they get it automatically
- **Execution history** — they can see every run, duration, output
- **Multi-machine** — one launcher shows all their runners across machines
- **No technical knowledge required** to run it

---

### Pricing Model (Simple)

| Tier | What | Price |
|------|------|-------|
| **Per automation** | You build + maintain one automation package | $50–200 one-time or $20–50/month |
| **Managed runner** | You set up runner on their machine/server, monitor it | $30–80/month per machine |
| **Platform license** | Launcher + runner for their team, you host the package registry | $100–300/month |

Start with **per-automation** — lowest friction, easiest to sell, and each one is a demo of the platform.

---

### Real Automation Examples to Build First

These are useful to yourself AND demonstrable to clients. Build them as proper `.tar.zst` packages with `manifest.yaml`:

#### 1. `disk-report` — Disk Usage Reporter
- Walk a directory, find top-20 largest folders/files
- Output a clean summary (text + optional HTML)
- **Why useful to you:** Run it on any machine before a demo to know what's there
- **Why clients want it:** "Where did my disk space go?" is universal

#### 2. `dev-env-snapshot` — Developer Environment Audit
- Detect installed tools: git, node, python, rustc, dotnet, docker — with versions
- Output JSON + human-readable report
- **Why useful to you:** Instant machine audit when onboarding a new client
- **Why clients want it:** "What's actually installed on this server?"

#### 3. `temp-cleaner` — Safe Temp File Cleaner
- Clean `%TEMP%`, browser caches, Windows Update cache
- Report MB freed, files removed
- Dry-run mode (show what would be deleted without deleting)
- **You already have this** — polish it, add dry-run, make it the flagship demo

#### 4. `log-archiver` — Log File Archiver
- Compress logs older than N days from a target directory
- Move to archive folder, report space saved
- **Why clients want it:** Every server accumulates logs; nobody manages them

#### 5. `git-branch-cleaner` — Stale Branch Cleaner
- List local git branches merged >30 days ago
- Interactive mode (confirm each) or auto mode
- **Why useful to you:** Run it on your own repos monthly

---

### Infrastructure Setup (Small Cost, Real Setup)

For a proper demo with a real remote runner:

| Component | Option | Cost |
|-----------|--------|------|
| Package registry | GitHub Releases (public or private repo) | $0–4/month |
| Remote Linux runner | Oracle Cloud Always Free (2 AMD VMs, permanent) | **$0** |
| Remote Windows runner | Your own machine as a Windows Service | $0 |
| Coordination (heartbeats) | File-based via shared dir (WSL2) or Syncthing | $0 |

**Recommended first setup:**
1. Your Windows machine: launcher GUI + Windows runner as a service
2. WSL2 on same machine: Linux runner (instant, zero cost)
3. GitHub Releases: package registry for your automation packages
4. Oracle Free Tier: real remote Linux VM when you want to demo "remote runner"

This gives you a 3-node demo (Windows native, WSL2 Linux, Oracle Linux) for $0/month.

---

### Demo Script (What You Show a Client)

1. Open launcher → **Browse tab** shows your automation packages from GitHub
2. Click `disk-report` → detail panel shows description, inputs, version
3. Click **[Run ▶]** → execution log streams in real time → summary appears
4. Switch to **Systems tab** → shows 2–3 connected runners with heartbeat status
5. Select a runner → assign `log-archiver` to run tonight at 2am
6. Next morning: Systems tab shows last run result, duration, output

That 5-minute demo sells the concept without any slides.
