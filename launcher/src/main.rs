mod artifact;
mod cli;
mod commands;
mod config;
mod error;
#[cfg(feature = "gui")]
mod gui;
mod providers;
mod services;
mod telemetry;
mod util;

use clap::Parser;
use tracing::{error, info};

use crate::cli::{Cli, Commands};
use crate::config::LauncherConfig;
use crate::error::LauncherError;
use crate::telemetry::{EventType, TelemetryEvent, TelemetryManager, init_logging};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        error!("{}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), LauncherError> {
    let mut config = match &cli.config {
        Some(path) => LauncherConfig::load_from(path)?,
        None => LauncherConfig::load()?,
    };

    if cli.offline {
        config.offline_mode = true;
    }

    init_logging(&config)?;
    info!("Automation Launcher starting");

    // GUI mode
    #[cfg(feature = "gui")]
    if matches!(cli.command, Some(Commands::Gui)) || cli.command.is_none() {
        let rt = tokio::runtime::Handle::current();
        let cfg = config.clone();
        gui::run_gui(cfg, rt).map_err(|e| LauncherError::Config(e))?;
        return Ok(());
    }

    let telemetry = TelemetryManager::new(config.logs_dir(), config.telemetry.enabled)
        .with_remote(config.telemetry.remote_endpoint.clone());
    telemetry.record(&TelemetryEvent::new(EventType::LauncherStartup))?;

    match cli.command {
        None => {
            println!("Automation Launcher v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Base directory: {}", config.base_dir.display());
            println!("Provider:      {:?}", config.provider);
            println!("Auth:          {:?}", config.auth);
            println!("Offline mode:  {}", config.offline_mode);
            println!("Cache limit:   {} MB", config.cache.max_size_mb);
            println!(
                "Telemetry:     {}",
                if config.telemetry.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!();
            println!("Run `launcher --help` for available commands.");
            println!("Run `launcher gui` to open the graphical interface.");
        }
        Some(Commands::List) => {
            commands::cmd_list(&config).await?;
        }
        Some(Commands::Search { query }) => {
            commands::cmd_search(&config, &query).await?;
        }
        Some(Commands::Download { tool, version }) => {
            commands::cmd_download(&config, &telemetry, &tool, version.as_deref()).await?;
        }
        Some(Commands::Run {
            tool,
            version,
            wait,
            args,
        }) => {
            commands::cmd_run(&config, &telemetry, &tool, version.as_deref(), wait, &args).await?;
        }
        Some(Commands::Cache { action }) => {
            commands::cmd_cache(&config, action)?;
        }
        Some(Commands::Config { action }) => {
            commands::cmd_config(&config, action)?;
        }
        Some(Commands::Fav { action }) => {
            commands::cmd_fav(&config, action)?;
        }
        #[cfg(feature = "gui")]
        Some(Commands::Gui) => unreachable!(),
    }

    // Flush telemetry at shutdown
    if telemetry.should_flush() {
        let _ = telemetry.flush().await;
    }

    info!("Automation Launcher finished");
    Ok(())
}
