use crate::artifact::ArtifactManifest;
use crate::config::LauncherConfig;
use crate::error::LauncherError;
use crate::services::cache::CacheManager;
use crate::services::execution::ExecutionManager;
use crate::telemetry::{EventType, TelemetryEvent, TelemetryManager};

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

    // Resolve version
    let resolved_version = match version {
        Some(v) => v.to_string(),
        None => {
            let entries = cache.list_entries()?;
            let mut tool_entries: Vec<_> = entries
                .iter()
                .filter(|e| e.tool_name == tool_name)
                .collect();
            tool_entries.sort_by(|a, b| b.last_used.cmp(&a.last_used));

            if let Some(entry) = tool_entries.first() {
                entry.version.clone()
            } else if !config.offline_mode {
                let manifest = fetch_manifest(config).await?;
                let artifact = manifest
                    .find_artifact(tool_name)
                    .ok_or_else(|| LauncherError::ArtifactNotFound(tool_name.to_string()))?;
                let latest = artifact.latest_version().ok_or_else(|| {
                    LauncherError::ArtifactNotFound(format!("{}: no versions", tool_name))
                })?;
                latest.version.clone()
            } else {
                return Err(LauncherError::ArtifactNotFound(format!(
                    "{}: not cached and offline mode is enabled",
                    tool_name
                )));
            }
        }
    };

    // Check cache
    let cached_path = match cache.get_cached_path(tool_name, &resolved_version) {
        Some(path) => path,
        None => {
            if config.offline_mode {
                return Err(LauncherError::ArtifactNotFound(format!(
                    "{} v{} not cached (offline mode)",
                    tool_name, resolved_version
                )));
            }
            println!(
                "{} v{} not cached, downloading...",
                tool_name, resolved_version
            );
            super::download::cmd_download(config, telemetry, tool_name, Some(&resolved_version)).await?;
            cache
                .get_cached_path(tool_name, &resolved_version)
                .ok_or_else(|| {
                    LauncherError::Cache("Failed to locate artifact after download".into())
                })?
        }
    };

    cache.touch(tool_name, &resolved_version)?;

    // Build launch config
    let mut launch_config = crate::artifact::LaunchConfig {
        executable: cached_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        args: if extra_args.is_empty() {
            None
        } else {
            Some(extra_args.to_vec())
        },
        env: None,
    };

    // Try to load launch config from manifest
    let manifest_path = config.metadata_dir().join("manifest.json");
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<ArtifactManifest>(&content) {
                if let Some(art) = manifest.find_artifact(tool_name) {
                    if let Some(ver) = art.find_version(&resolved_version) {
                        launch_config.executable = ver.launch.executable.clone();
                        if extra_args.is_empty() {
                            launch_config.args = ver.launch.args.clone();
                        }
                        launch_config.env = ver.launch.env.clone();
                    }
                }
            }
        }
    }

    let executable_path = cached_path
        .parent()
        .unwrap_or(&cached_path)
        .join(&launch_config.executable);

    println!("Running {} v{} ...", tool_name, resolved_version);

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
                if code == 0 {
                    telemetry.record(
                        &TelemetryEvent::new(EventType::ExecutionSuccess)
                            .with_tool(tool_name, &resolved_version)
                            .with_status("success"),
                    )?;
                } else {
                    telemetry.record(
                        &TelemetryEvent::new(EventType::ExecutionFailure)
                            .with_tool(tool_name, &resolved_version)
                            .with_status(&format!("exit_code:{}", code)),
                    )?;
                }
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
