use crate::config::LauncherConfig;
use crate::error::LauncherError;
use crate::services::cache::CacheManager;
use crate::services::download::DownloadManager;
use crate::services::extract;
use crate::telemetry::{EventType, TelemetryEvent, TelemetryManager};

use super::fetch_manifest;

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
        Some(v) => artifact.find_version(v).ok_or_else(|| {
            LauncherError::VersionNotFound {
                tool: tool_name.to_string(),
                version: v.to_string(),
            }
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

    let dm = DownloadManager::new(config.downloads_dir());
    let downloaded_path = dm
        .download(&artifact_version.download_url, &artifact_version.sha256)
        .await?;

    // Store in cache
    let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
    cache.init()?;

    // Handle zip extraction
    if extract::is_zip(&downloaded_path) {
        let extract_dir = config.cache_dir().join(tool_name).join(ver);
        let files = extract::extract_zip(&downloaded_path, &extract_dir)?;
        if let Some(exe) = files.iter().find(|f| {
            f.extension()
                .map(|e| e == "exe" || e == "ps1" || e == "bat")
                .unwrap_or(false)
        }) {
            cache.store(tool_name, ver, exe)?;
        } else if let Some(first) = files.first() {
            cache.store(tool_name, ver, first)?;
        }
    } else {
        cache.store(tool_name, ver, &downloaded_path)?;
    }

    // Clean up download
    let _ = std::fs::remove_file(&downloaded_path);

    telemetry.record(
        &TelemetryEvent::new(EventType::DownloadComplete).with_tool(tool_name, ver),
    )?;

    println!("Downloaded and cached: {} v{}", tool_name, ver);
    Ok(())
}
