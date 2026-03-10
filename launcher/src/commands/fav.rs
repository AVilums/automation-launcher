use crate::cli::FavAction;
use crate::config::LauncherConfig;
use crate::error::LauncherError;
use crate::services::favorites::Favorites;

pub fn cmd_fav(config: &LauncherConfig, action: FavAction) -> Result<(), LauncherError> {
    let fav_path = Favorites::file_path(&config.base_dir);
    let mut favorites = Favorites::load(&fav_path)?;

    match action {
        FavAction::List => {
            if favorites.items.is_empty() {
                println!("No favorites.");
                return Ok(());
            }
            println!("{:<25} {}", "TOOL", "PINNED VERSION");
            println!("{}", "-".repeat(40));
            for item in &favorites.items {
                let ver = item
                    .pinned_version
                    .as_deref()
                    .unwrap_or("latest");
                println!("{:<25} {}", item.tool_name, ver);
            }
        }
        FavAction::Add { tool, version } => {
            favorites.add(&tool, version.clone());
            favorites.save(&fav_path)?;
            let ver_str = version.as_deref().unwrap_or("latest");
            println!("Added {} (version: {}) to favorites", tool, ver_str);
        }
        FavAction::Remove { tool } => {
            favorites.remove(&tool);
            favorites.save(&fav_path)?;
            println!("Removed {} from favorites", tool);
        }
    }

    Ok(())
}
