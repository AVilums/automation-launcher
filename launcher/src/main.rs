mod gui;

use tracing::{info};
use config::LauncherConfig;
use telemetry::{init_logging};

#[tokio::main]
async fn main() {
    let config = LauncherConfig::load().unwrap_or_default();
    init_logging(&config).expect("logging initialization failed");

    info!("Automation Launcher starting");
    let rt = tokio::runtime::Handle::current();
    gui::run_gui(config, rt).expect("GUI failed to start");
}
