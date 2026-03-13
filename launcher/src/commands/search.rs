use config::LauncherConfig;
use domain::LauncherError;

pub async fn cmd_search(config: &LauncherConfig, query: &str) -> Result<(), LauncherError> {
    let manifest = runtime::fetch_manifest(config).await?;
    let results = manifest.search(query);

    if results.is_empty() {
        println!("No artifacts matching '{}'", query);
        return Ok(());
    }

    println!("{:<25} {:<12} {}", "NAME", "LATEST", "DESCRIPTION");
    println!("{}", "-".repeat(70));

    for artifact in &results {
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
    println!("{} result(s) for '{}'", results.len(), query);
    Ok(())
}
