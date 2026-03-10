use crate::config::LauncherConfig;
use crate::error::LauncherError;

use super::fetch_manifest;

pub async fn cmd_list(config: &LauncherConfig) -> Result<(), LauncherError> {
    let manifest = fetch_manifest(config).await?;

    if manifest.artifacts.is_empty() {
        println!("No artifacts available.");
        return Ok(());
    }

    println!("{:<25} {:<12} {}", "NAME", "LATEST", "DESCRIPTION");
    println!("{}", "-".repeat(70));

    for artifact in &manifest.artifacts {
        let latest = artifact
            .latest_version()
            .map(|v| v.version.as_str())
            .unwrap_or("—");
        println!(
            "{:<25} {:<12} {}",
            artifact.name, latest, artifact.description
        );
    }

    println!();
    println!("{} artifact(s) available", manifest.artifacts.len());
    Ok(())
}
