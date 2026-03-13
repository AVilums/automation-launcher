use clap::{Parser, Subcommand};
use config::LauncherConfig;
use domain::LauncherError;
use scheduler::{Schedule, ScheduledTask, Scheduler};
use std::path::PathBuf;
use telemetry::{EventType, TelemetryEvent, TelemetryManager, init_logging};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "runner")]
#[command(about = "Automation Runner — headless tool execution and scheduling")]
#[command(version)]
struct Cli {
    /// Path to a custom configuration file
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Run in offline mode (use cached artifacts only)
    #[arg(long, global = true)]
    offline: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a tool directly
    Run {
        /// Tool name
        tool: String,

        /// Specific version (defaults to latest)
        #[arg(long)]
        version: Option<String>,

        /// Wait for the process to finish
        #[arg(long)]
        wait: bool,

        /// Extra arguments passed to the tool
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Schedule management
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },

    /// Run all due scheduled tasks and exit
    Tick,

    /// Run the scheduler loop continuously
    Daemon {
        /// Check interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,
    },
}

#[derive(Subcommand)]
enum ScheduleAction {
    /// List all scheduled tasks
    List,
    /// Add a scheduled task (interval-based)
    Add {
        /// Task ID
        #[arg(long)]
        id: String,
        /// Tool name
        tool: String,
        /// Interval in seconds
        #[arg(long)]
        interval: u64,
        /// Specific version
        #[arg(long)]
        version: Option<String>,
    },
    /// Remove a scheduled task
    Remove {
        /// Task ID
        id: String,
    },
}

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
    info!("Automation Runner starting");

    let telemetry = TelemetryManager::new(config.logs_dir(), config.telemetry.enabled)
        .with_remote(config.telemetry.remote_endpoint.clone());
    telemetry.record(&TelemetryEvent::new(EventType::LauncherStartup))?;

    match cli.command {
        Commands::Run {
            tool,
            version,
            wait,
            args,
        } => {
            runtime::cmd_run(&config, &telemetry, &tool, version.as_deref(), wait, &args).await?;
        }
        Commands::Schedule { action } => match action {
            ScheduleAction::List => {
                let scheduler = Scheduler::new(&config.base_dir);
                scheduler.init()?;
                let tasks = scheduler.list_tasks()?;
                if tasks.is_empty() {
                    println!("No scheduled tasks.");
                } else {
                    for task in &tasks {
                        println!(
                            "  {} | {} | {:?} | enabled={} | last_run={:?}",
                            task.id, task.tool_name, task.schedule, task.enabled, task.last_run
                        );
                    }
                }
            }
            ScheduleAction::Add {
                id,
                tool,
                interval,
                version,
            } => {
                let scheduler = Scheduler::new(&config.base_dir);
                scheduler.init()?;
                let task = ScheduledTask {
                    id: id.clone(),
                    tool_name: tool.clone(),
                    version,
                    schedule: Schedule::Interval { seconds: interval },
                    extra_args: vec![],
                    enabled: true,
                    last_run: None,
                };
                scheduler.add_task(task)?;
                println!("Scheduled task '{}' added for tool '{}'", id, tool);
            }
            ScheduleAction::Remove { id } => {
                let scheduler = Scheduler::new(&config.base_dir);
                scheduler.init()?;
                scheduler.remove_task(&id)?;
                println!("Scheduled task '{}' removed", id);
            }
        },
        Commands::Tick => {
            let scheduler = Scheduler::new(&config.base_dir);
            scheduler.init()?;
            let count = scheduler.run_due_tasks(&config, &telemetry).await?;
            println!("Ran {} due task(s)", count);
        }
        Commands::Daemon { interval } => {
            let scheduler = Scheduler::new(&config.base_dir);
            scheduler.init()?;
            println!("Runner daemon started (interval: {}s)", interval);
            loop {
                let count = scheduler.run_due_tasks(&config, &telemetry).await?;
                if count > 0 {
                    info!("Ran {} due task(s)", count);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            }
        }
    }

    if telemetry.should_flush() {
        let _ = telemetry.flush().await;
    }

    info!("Automation Runner finished");
    Ok(())
}
