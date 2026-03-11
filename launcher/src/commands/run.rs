use crate::artifact::{ArtifactManifest, LaunchConfig, LocalArtifactManifest};
use crate::config::LauncherConfig;
use crate::error::LauncherError;
use crate::services::cache::CacheManager;
use crate::services::execution::ExecutionManager;
use crate::telemetry::{EventType, TelemetryEvent, TelemetryManager};
use std::path::{Path, PathBuf};
use tracing::info;

use super::fetch_manifest;

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
    info!("Starting {} v{} from {}", tool_name, resolved_version, executable_path.display());

    telemetry.record(
        &TelemetryEvent::new(EventType::ExecutionStart)
            .with_tool(tool_name, &resolved_version),
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
                let event_type = if code == 0 { EventType::ExecutionSuccess } else { EventType::ExecutionFailure };
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

    // Try most recently used cached version
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

    // Fetch latest from remote
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
        info!("Found {} v{} in cache at {}", tool_name, version, path.display());
        return Ok(path);
    }

    if config.offline_mode {
        return Err(LauncherError::ArtifactNotFound(format!(
            "{} v{} not cached (offline mode)",
            tool_name, version
        )));
    }

    info!("{} v{} not cached, downloading...", tool_name, version);
    super::download::cmd_download(config, telemetry, tool_name, Some(version)).await?;

    cache.get_cached_path(tool_name, version).ok_or_else(|| {
        LauncherError::Cache("Failed to locate artifact after download".into())
    })
}

/// Build launch configuration by reading the manifest shipped inside the cached artifact.
pub(crate) fn build_launch_config(
    tool_name: &str,
    version: &str,
    cached_path: &Path,
    extra_args: &[String],
) -> Result<LaunchConfig, LauncherError> {
    let args = if extra_args.is_empty() { None } else { Some(extra_args.to_vec()) };

    let manifest_path = if cached_path.is_dir() {
        cached_path.join("manifest.json")
    } else {
        cached_path.parent().unwrap_or(cached_path).join("manifest.json")
    };

    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        // Local manifest (shipped in artifact ZIPs)
        if let Ok(local) = serde_json::from_str::<LocalArtifactManifest>(&content) {
            return Ok(LaunchConfig {
                executable: derive_executable(&local.name, local.entry_point.as_deref()),
                args,
                env: None,
            });
        }

        // Full artifact manifest (remote-style, less common in cache)
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

        tracing::warn!("Could not parse manifest at {}", manifest_path.display());
    }

    // Default: assume <tool_name>.exe
    Ok(LaunchConfig {
        executable: format!("{}.exe", tool_name),
        args,
        env: None,
    })
}

/// Derive the executable filename from the artifact name.
/// The `entry_point` field in local manifests refers to the *source* file (e.g. "main.au3"),
/// so we only use it when it already looks like a runnable binary.
pub(crate) fn derive_executable(name: &str, entry_point: Option<&str>) -> String {
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

/// Resolve the full path to the executable within the cached artifact directory.
pub(crate) fn resolve_executable(
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

    // Try the configured executable name
    let candidate = cached_path.join(&launch_config.executable);
    if candidate.exists() {
        return Ok(candidate);
    }

    // Try appending .exe if not already present
    if !launch_config.executable.ends_with(".exe") {
        let with_exe = cached_path.join(format!("{}.exe", launch_config.executable));
        if with_exe.exists() {
            return Ok(with_exe);
        }
    }

    // Scan directory for a matching executable
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

/// Scan a directory for the most likely executable, preferring an exact tool name match.
pub(crate) fn find_executable_in_dir(dir: &Path, tool_name: &str) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    let expected = format!("{}.exe", tool_name).to_lowercase();

    // Prefer exact match
    for entry in &entries {
        if entry.file_name().to_string_lossy().to_lowercase() == expected {
            return Some(entry.path());
        }
    }

    // Fall back to first runnable file
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.ends_with(".exe") || name.ends_with(".ps1") || name.ends_with(".bat") {
            return Some(entry.path());
        }
    }

    None
}
