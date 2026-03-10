### 1. Why does the launcher just exit and bootstrap fails?

**Launcher:** Right now `main.rs` just logs "starting" and "ready" then exits — there's no actual workflow yet. Phase 1 built the *modules* (download, cache, execution, etc.) but didn't wire them into a working CLI flow. That's expected at this stage.

**Bootstrap:** It tries to connect to `https://localhost/update/metadata.json` which doesn't exist. The bootstrap needs a real update server URL or a way to skip the update check for local development. You can fix this by editing `bootstrap/src/config.rs` and changing the default `update_url`, or by adding a `--skip-update` flag.

### 2. Why are tests inside the source files instead of separate?

In Rust, there are **two common patterns**:

- **Unit tests** (`#[cfg(test)] mod tests` inside the source file) — This is the idiomatic Rust way for testing private functions. The `#[cfg(test)]` attribute means the test code is only compiled when running `cargo test`, never in release builds. It's not "worse" — it's actually the Rust community standard.

- **Integration tests** (`tests/` directory at crate root) — These test the *public API* of your crate as an external consumer would. They can only access `pub` items.

**Best practice in Rust:**
- Keep unit tests in-file (as they are now) for testing internal logic
- Add a `tests/` folder for integration tests that test the public API end-to-end
- Example structure:
```
launcher/
    src/
        config.rs      # has #[cfg(test)] mod tests inside
        cache.rs       # has #[cfg(test)] mod tests inside
    tests/
        integration_download.rs   # tests public API
        integration_cache.rs      # tests public API
```

### 3. Why no folder structure between files?

Right now all modules are flat in `launcher/src/`:
```
launcher/src/
    main.rs
    config.rs
    cache.rs
    download.rs
    ...
```

As the project grows, you should organize into **subdirectories** (Rust calls them nested modules):
```
launcher/src/
    main.rs
    config/
        mod.rs          # or config.rs at parent level
        settings.rs
    cache/
        mod.rs
        eviction.rs
        index.rs
    download/
        mod.rs
        manager.rs
        resume.rs
    provider/
        mod.rs
        github.rs
        http.rs
        mock.rs
```

In Rust, you create a folder with a `mod.rs` file inside it, and that becomes a module with sub-modules. This is the right time to start restructuring.

### 4. What are the next steps?

Here's a beginner-friendly roadmap:

#### Step A — Make the launcher actually do something (CLI mode)
Add a simple command-line interface so you can test the modules you built:
```
launcher.exe list              # list artifacts from a provider
launcher.exe download <tool>   # download a tool
launcher.exe run <tool>        # execute a cached tool
launcher.exe cache status      # show cache usage
```

Use the `clap` crate for argument parsing — add it to `launcher/Cargo.toml`:
```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
```

#### Step B — Fix the bootstrap for local dev
Add a `--skip-update` flag or a local dev mode so bootstrap just launches the launcher without checking for updates.

#### Step C — Create a sample artifact
Create a simple test artifact (a `.bat` or `.ps1` script), host it somewhere (even a local folder or GitHub release), and configure a provider to serve it. This lets you test the full download → verify → cache → execute pipeline.

#### Step D — Add integration tests
Create `launcher/tests/` and `bootstrap/tests/` directories with end-to-end tests using the mock provider.

#### Step E — Restructure into folders
Split larger modules into sub-modules as described above.

### Quick Rust Tips for You

- `cargo clippy` — gives you better warnings and style suggestions than the compiler
- `cargo fmt` — auto-formats your code
- `cargo test -- --nocapture` — shows `println!` output during tests
- `cargo doc --open` — generates and opens HTML documentation for your code
- The Rust Book (https://doc.rust-lang.org/book/) is excellent for learning

### Summary

Your Phase 1 foundation is solid — all the core modules work and tests pass. The next priority is wiring everything together into a usable CLI so you can actually run automations. Start with Step A (CLI) and Step B (bootstrap dev mode), then iterate from there.
