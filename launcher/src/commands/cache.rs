use crate::cli::CacheAction;
use config::LauncherConfig;
use domain::{LauncherError, format_bytes};
use storage::cache::CacheManager;

pub fn cmd_cache(config: &LauncherConfig, action: CacheAction) -> Result<(), LauncherError> {
    let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
    cache.init()?;

    match action {
        CacheAction::Status => {
            let entries = cache.list_entries()?;
            let total = cache.total_size()?;
            let limit = config.cache.max_size_mb * 1024 * 1024;

            println!("Cache Status");
            println!("  Location:    {}", config.cache_dir().display());
            println!("  Entries:     {}", entries.len());
            println!("  Used:        {}", format_bytes(total));
            println!("  Limit:       {}", format_bytes(limit));
            println!(
                "  Available:   {}",
                format_bytes(limit.saturating_sub(total))
            );
        }
        CacheAction::List => {
            let entries = cache.list_entries()?;

            if entries.is_empty() {
                println!("Cache is empty.");
                return Ok(());
            }

            println!(
                "{:<25} {:<12} {:<12} {}",
                "TOOL", "VERSION", "SIZE", "LAST USED"
            );
            println!("{}", "-".repeat(70));

            for entry in &entries {
                println!(
                    "{:<25} {:<12} {:<12} {}",
                    entry.tool_name,
                    entry.version,
                    format_bytes(entry.size_bytes),
                    entry.last_used.format("%Y-%m-%d %H:%M")
                );
            }

            println!();
            println!("{} cached artifact(s)", entries.len());
        }
        CacheAction::Remove { tool, version } => {
            cache.remove(&tool, &version)?;
            println!("Removed {} v{} from cache", tool, version);
        }
        CacheAction::Clear => {
            let entries = cache.list_entries()?;
            let count = entries.len();
            for entry in &entries {
                let _ = cache.remove(&entry.tool_name, &entry.version);
            }
            println!("Cleared {} cached artifact(s)", count);
        }
    }

    Ok(())
}
