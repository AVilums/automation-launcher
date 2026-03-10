mod config;
mod error;
mod killswitch;
mod updater;

use clap::Parser;
use error::BootstrapError;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "bootstrap")]
#[command(about = "Automation Launcher Bootstrap — updater and launcher manager")]
#[command(version)]
struct Cli {
    /// Skip checking for launcher updates
    #[arg(long)]
    skip_update: bool,

    /// Skip kill-switch check
    #[arg(long)]
    skip_killswitch: bool,

    /// Do not launch the main application after update check
    #[arg(long)]
    no_launch: bool,

    /// Force re-download of the launcher even if up to date
    #[arg(long)]
    force_update: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .init();

    if let Err(e) = run(cli).await {
        error!("{}", e);
        eprintln!("Bootstrap error: {}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), BootstrapError> {
    info!("Bootstrap starting");

    let config = config::BootstrapConfig::load()?;
    info!("Configuration loaded");

    // Kill-switch check
    if !cli.skip_killswitch {
        match killswitch::check_killswitch(&config).await {
            Ok(Some(ks)) if ks.enabled => {
                info!("Kill-switch is active");
                killswitch::enforce_killswitch(&config, &ks)?;
                return Ok(());
            }
            Ok(Some(_)) => {
                info!("Kill-switch is not active");
            }
            Ok(None) => {
                info!("No kill-switch configured, skipping");
            }
            Err(e) => {
                warn!("Kill-switch check failed (continuing): {}", e);
            }
        }
    } else {
        info!("Kill-switch check skipped (--skip-killswitch)");
    }

    // Update check
    if !cli.skip_update {
        match updater::check_for_update(&config).await {
            Ok(needs_update) => {
                if needs_update || cli.force_update {
                    let reason = if cli.force_update {
                        "forced update"
                    } else {
                        "update available"
                    };
                    info!("Downloading launcher ({})", reason);
                    println!("Updating launcher...");

                    match updater::perform_update(&config).await {
                        Ok(()) => {
                            info!("Launcher updated successfully");
                            println!("Launcher updated successfully.");
                        }
                        Err(e) => {
                            warn!("Update failed: {}", e);
                            eprintln!("Warning: Update failed ({}). Trying to launch existing version.", e);

                            if !config.launcher_path.exists() {
                                return Err(BootstrapError::LauncherNotFound(
                                    config.launcher_path.display().to_string(),
                                ));
                            }
                        }
                    }
                } else {
                    info!("Launcher is up to date");
                }
            }
            Err(e) => {
                warn!("Update check failed: {}", e);
                eprintln!("Warning: Could not check for updates ({}). Trying to launch existing version.", e);

                if !config.launcher_path.exists() {
                    return Err(BootstrapError::LauncherNotFound(
                        config.launcher_path.display().to_string(),
                    ));
                }
            }
        }
    } else {
        info!("Update check skipped (--skip-update)");
    }

    // Launch
    if !cli.no_launch {
        if config.launcher_path.exists() {
            info!("Launching main application");
            updater::launch_launcher(&config)?;
            println!("Launcher started.");
        } else {
            eprintln!(
                "Launcher not found at {}. Run without --skip-update to download it.",
                config.launcher_path.display()
            );
            return Err(BootstrapError::LauncherNotFound(
                config.launcher_path.display().to_string(),
            ));
        }
    } else {
        info!("Launch skipped (--no-launch)");
    }

    info!("Bootstrap completed");
    Ok(())
}
