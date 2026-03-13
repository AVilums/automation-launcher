use crate::cli::ConfigAction;
use config::LauncherConfig;
use domain::LauncherError;

pub fn cmd_config(config: &LauncherConfig, action: ConfigAction) -> Result<(), LauncherError> {
    match action {
        ConfigAction::Show => {
            let json = serde_json::to_string_pretty(config)?;
            println!("{}", json);
        }
        ConfigAction::Path => {
            let config_path = config.base_dir.join("config").join("launcher.json");
            println!("{}", config_path.display());
        }
        ConfigAction::Reset => {
            let config = LauncherConfig::load()?;
            config.save()?;
            println!("Configuration reset to defaults.");
            println!(
                "Config file: {}",
                config
                    .base_dir
                    .join("config")
                    .join("launcher.json")
                    .display()
            );
        }
    }

    Ok(())
}
