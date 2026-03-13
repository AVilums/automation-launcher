use config::LauncherConfig;
use domain::LauncherError;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

pub fn init_logging(config: &LauncherConfig) -> Result<(), LauncherError> {
    let logs_dir = config.logs_dir();
    std::fs::create_dir_all(&logs_dir)?;

    let file_appender = rolling::daily(&logs_dir, "launcher.log");

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with(
            fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_writer(file_appender)
                .json(),
        )
        .with(
            fmt::layer()
                .with_target(false)
                .with_level(true)
                .with_writer(std::io::stdout),
        );

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| LauncherError::Logging(e.to_string()))?;

    Ok(())
}
