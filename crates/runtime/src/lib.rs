use artifact::{ArtifactManifest, LaunchConfig, LocalArtifactManifest};
use config::{AuthConfig, LauncherConfig, ProviderConfig};
use domain::LauncherError;
use execution::ExecutionManager;
use network::DownloadManager;
use provider::{ArtifactProvider, GitHubProvider, HttpProvider};
use storage::cache::CacheManager;
use storage::extract;
use telemetry::{EventType, TelemetryEvent, TelemetryManager};
use std::path::{Path, PathBuf};
use tracing::info;

/// Fetch the artifact manifest (remote or cached).
pub async fn fetch_manifest(config: &LauncherConfig) -> Result<ArtifactManifest, LauncherError> {
    if config.offline_mode {
        let manifest_path = config.metadata_dir().join("manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: ArtifactManifest = serde_json::from_str(&content)?;
            return Ok(manifest);
        }
        return Err(LauncherError::Provider(
            "Offline mode: no cached manifest available".into(),
        ));
    }

    let manifest = match &config.provider {
        ProviderConfig::GitHub { owner, repo } => {
            let token = match &config.auth {
                AuthConfig::ApiKey { key } => Some(key.clone()),
                _ => None,
            };
            let provider = GitHubProvider::new(owner.clone(), repo.clone(), token);
            provider.fetch_manifest().await?
        }
        ProviderConfig::Http { base_url } => {
            let provider = HttpProvider::new(base_url.clone());
            provider.fetch_manifest().await?
        }
    };

    // Cache the manifest for offline use
    let metadata_dir = config.metadata_dir();
    std::fs::create_dir_all(&metadata_dir)?;
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(metadata_dir.join("manifest.json"), manifest_json)?;

    Ok(manifest)
}

/// Download an artifact and store it in the cache.
pub async fn download_and_cache(
    config: &LauncherConfig,
    tool_name: &str,
    version: &str,
    download_url: &str,
    sha256: &str,
) -> Result<(), LauncherError> {
    let token = match &config.auth {
        AuthConfig::ApiKey { key } => Some(key.clone()),
        _ => None,
    };
    let dm = DownloadManager::new(config.downloads_dir()).with_token(token);
    let downloaded_path = dm.download(download_url, sha256).await?;

    let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
    cache.init()?;

    if extract::is_zip(&downloaded_path) {
        let extract_dir = config
            .cache_dir()
            .join(".tmp_extract")
            .join(tool_name)
            .join(version);
        let files = extract::extract_zip(&downloaded_path, &extract_dir)?;

        let has_runnable = files.iter().any(|f| {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            name == format!("{}.exe", tool_name).to_lowercase()
                || f.extension()
                    .map(|e| e == "exe" || e == "ps1" || e == "bat")
                    .unwrap_or(false)
        });

        if has_runnable {
            cache.store(tool_name, version, &extract_dir)?;
        } else if let Some(first) = files.first() {
            cache.store(tool_name, version, first)?;
        }

        let _ = std::fs::remove_dir_all(&extract_dir);
    } else {
        cache.store(tool_name, version, &downloaded_path)?;
    }

    let _ = std::fs::remove_file(&downloaded_path);
    Ok(())
}

/// Download command: fetch manifest, find artifact, download.
pub async fn cmd_download(
    config: &LauncherConfig,
    telemetry: &TelemetryManager,
    tool_name: &str,
    version: Option<&str>,
) -> Result<(), LauncherError> {
    if config.offline_mode {
        return Err(LauncherError::Download(
            "Cannot download in offline mode".into(),
        ));
    }

    let manifest = fetch_manifest(config).await?;

    let artifact = manifest
        .find_artifact(tool_name)
        .ok_or_else(|| LauncherError::ArtifactNotFound(tool_name.to_string()))?;

    let artifact_version = match version {
        Some(v) => artifact.find_version(v).ok_or_else(|| LauncherError::VersionNotFound {
            tool: tool_name.to_string(),
            version: v.to_string(),
        })?,
        None => artifact.latest_version().ok_or_else(|| {
            LauncherError::ArtifactNotFound(format!("{}: no versions available", tool_name))
        })?,
    };

    let ver = &artifact_version.version;
    println!(
        "Downloading {} v{} ({} bytes)...",
        tool_name, ver, artifact_version.file_size
    );

    telemetry.record(
        &TelemetryEvent::new(EventType::DownloadStart).with_tool(tool_name, ver),
    )?;

    download_and_cache(
        config,
        tool_name,
        ver,
        &artifact_version.download_url,
        &artifact_version.sha256,
    )
    .await?;

    telemetry.record(
        &TelemetryEvent::new(EventType::DownloadComplete).with_tool(tool_name, ver),
    )?;

    println!("Downloaded and cached: {} v{}", tool_name, ver);
    Ok(())
}

/// Run command: resolve version, ensure cached, execute.
pub async fn cmd_run(
    config: &LauncherConfig,
    telemetry: &TelemetryManager,
    tool_name: &str,
    version: Option<&str>,
    wait: bool,
    extra_args: &[String],
) -> Result<(), LauncherError> {
    let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
    cache.init()?;

    let resolved_version = resolve_version(&cache, config, tool_name, version).await?;
    let cached_path = ensure_cached(&cache, config, telemetry, tool_name, &resolved_version).await?;

    cache.touch(tool_name, &resolved_version)?;

    let launch_config = build_launch_config(tool_name, &resolved_version, &cached_path, extra_args)?;
    let executable_path = resolve_executable(&cached_path, &launch_config, tool_name)?;

    println!("Running {} v{} ...", tool_name, resolved_version);
    info!(
        "Starting {} v{} from {}",
        tool_name,
        resolved_version,
        executable_path.display()
    );

    telemetry.record(
        &TelemetryEvent::new(EventType::ExecutionStart).with_tool(tool_name, &resolved_version),
    )?;

    let exec_manager = ExecutionManager::new();
    let result = if wait {
        exec_manager.execute_and_wait(&executable_path, &launch_config)
    } else {
        exec_manager.execute(&executable_path, &launch_config)
    };

    match result {
        Ok(exec_result) => {
            if let Some(code) = exec_result.exit_code {
                println!("Process exited with code {}", code);
                let event_type = if code == 0 {
                    EventType::ExecutionSuccess
                } else {
                    EventType::ExecutionFailure
                };
                telemetry.record(
                    &TelemetryEvent::new(event_type)
                        .with_tool(tool_name, &resolved_version)
                        .with_status(&format!("exit_code:{}", code)),
                )?;
            } else {
                println!("Process started (PID {})", exec_result.pid);
                telemetry.record(
                    &TelemetryEvent::new(EventType::ExecutionSuccess)
                        .with_tool(tool_name, &resolved_version)
                        .with_status("launched"),
                )?;
            }
            Ok(())
        }
        Err(e) => {
            telemetry.record(
                &TelemetryEvent::new(EventType::ExecutionFailure)
                    .with_tool(tool_name, &resolved_version)
                    .with_details(&e.to_string()),
            )?;
            Err(e)
        }
    }
}

async fn resolve_version(
    cache: &CacheManager,
    config: &LauncherConfig,
    tool_name: &str,
    version: Option<&str>,
) -> Result<String, LauncherError> {
    if let Some(v) = version {
        return Ok(v.to_string());
    }

    let mut entries: Vec<_> = cache
        .list_entries()?
        .into_iter()
        .filter(|e| e.tool_name == tool_name)
        .collect();
    entries.sort_by(|a, b| b.last_used.cmp(&a.last_used));

    if let Some(entry) = entries.first() {
        return Ok(entry.version.clone());
    }

    if config.offline_mode {
        return Err(LauncherError::ArtifactNotFound(format!(
            "{}: not cached and offline mode is enabled",
            tool_name
        )));
    }

    let manifest = fetch_manifest(config).await?;
    let artifact = manifest
        .find_artifact(tool_name)
        .ok_or_else(|| LauncherError::ArtifactNotFound(tool_name.to_string()))?;
    let latest = artifact
        .latest_version()
        .ok_or_else(|| LauncherError::ArtifactNotFound(format!("{}: no versions", tool_name)))?;
    Ok(latest.version.clone())
}

async fn ensure_cached(
    cache: &CacheManager,
    config: &LauncherConfig,
    telemetry: &TelemetryManager,
    tool_name: &str,
    version: &str,
) -> Result<PathBuf, LauncherError> {
    if let Some(path) = cache.get_cached_path(tool_name, version) {
        info!(
            "Found {} v{} in cache at {}",
            tool_name,
            version,
            path.display()
        );
        return Ok(path);
    }

    if config.offline_mode {
        return Err(LauncherError::ArtifactNotFound(format!(
            "{} v{} not cached (offline mode)",
            tool_name, version
        )));
    }

    info!("{} v{} not cached, downloading...", tool_name, version);
    cmd_download(config, telemetry, tool_name, Some(version)).await?;

    cache
        .get_cached_path(tool_name, version)
        .ok_or_else(|| LauncherError::Cache("Failed to locate artifact after download".into()))
}

pub fn build_launch_config(
    tool_name: &str,
    version: &str,
    cached_path: &Path,
    extra_args: &[String],
) -> Result<LaunchConfig, LauncherError> {
    let args = if extra_args.is_empty() {
        None
    } else {
        Some(extra_args.to_vec())
    };

    let manifest_path = if cached_path.is_dir() {
        cached_path.join("manifest.json")
    } else {
        cached_path
            .parent()
            .unwrap_or(cached_path)
            .join("manifest.json")
    };

    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(local) = serde_json::from_str::<LocalArtifactManifest>(&content) {
            return Ok(LaunchConfig {
                executable: derive_executable(&local.name, local.entry_point.as_deref()),
                args,
                env: None,
            });
        }

        if let Ok(manifest) = serde_json::from_str::<ArtifactManifest>(&content) {
            if let Some(launch) = manifest
                .find_artifact(tool_name)
                .and_then(|a| a.find_version(version).or_else(|| a.latest_version()))
                .map(|v| &v.launch)
            {
                return Ok(LaunchConfig {
                    executable: launch.executable.clone(),
                    args: args.or_else(|| launch.args.clone()),
                    env: launch.env.clone(),
                });
            }
        }
    }

    Ok(LaunchConfig {
        executable: format!("{}.exe", tool_name),
        args,
        env: None,
    })
}

pub fn derive_executable(name: &str, entry_point: Option<&str>) -> String {
    if let Some(ep) = entry_point {
        let is_runnable = ep.ends_with(".exe")
            || ep.ends_with(".ps1")
            || ep.ends_with(".bat")
            || ep.ends_with(".cmd");
        if is_runnable {
            return ep.to_string();
        }
    }
    format!("{}.exe", name)
}

pub fn resolve_executable(
    cached_path: &Path,
    launch_config: &LaunchConfig,
    tool_name: &str,
) -> Result<PathBuf, LauncherError> {
    if !cached_path.is_dir() {
        if cached_path.exists() {
            return Ok(cached_path.to_path_buf());
        }
        return Err(LauncherError::Execution(format!(
            "Executable not found: {}",
            cached_path.display()
        )));
    }

    let candidate = cached_path.join(&launch_config.executable);
    if candidate.exists() {
        return Ok(candidate);
    }

    if !launch_config.executable.ends_with(".exe") {
        let with_exe = cached_path.join(format!("{}.exe", launch_config.executable));
        if with_exe.exists() {
            return Ok(with_exe);
        }
    }

    if let Some(found) = find_executable_in_dir(cached_path, tool_name) {
        info!("Auto-detected executable: {}", found.display());
        return Ok(found);
    }

    Err(LauncherError::Execution(format!(
        "Executable not found in {}: expected {}",
        cached_path.display(),
        launch_config.executable
    )))
}

pub fn find_executable_in_dir(dir: &Path, tool_name: &str) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    let expected = format!("{}.exe", tool_name).to_lowercase();

    for entry in &entries {
        if entry.file_name().to_string_lossy().to_lowercase() == expected {
            return Some(entry.path());
        }
    }

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.ends_with(".exe") || name.ends_with(".ps1") || name.ends_with(".bat") {
            return Some(entry.path());
        }
    }

    None
}
