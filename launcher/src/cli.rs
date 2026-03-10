use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "launcher")]
#[command(about = "Automation Launcher — secure tool distribution and execution")]
#[command(version)]
pub struct Cli {
    /// Path to a custom configuration file
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Run in offline mode (use cached artifacts only)
    #[arg(long, global = true)]
    pub offline: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List available automation tools
    List,

    /// Search for tools by name, description, or tag
    Search {
        /// Search query
        query: String,
    },

    /// Download a tool artifact
    Download {
        /// Tool name
        tool: String,

        /// Specific version (defaults to latest)
        #[arg(long)]
        version: Option<String>,
    },

    /// Run a cached automation tool
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

    /// Cache management commands
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Show or update configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage favorite tools
    Fav {
        #[command(subcommand)]
        action: FavAction,
    },

    /// Launch the graphical interface (requires --features gui)
    #[cfg(feature = "gui")]
    Gui,
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Show cache status (entries, total size, limit)
    Status,
    /// List all cached artifacts
    List,
    /// Remove a specific cached artifact
    Remove {
        /// Tool name
        tool: String,
        /// Tool version
        version: String,
    },
    /// Clear the entire cache
    Clear,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Display current configuration
    Show,
    /// Show the configuration file path
    Path,
    /// Reset configuration to defaults
    Reset,
}

#[derive(Subcommand)]
pub enum FavAction {
    /// List favorite tools
    List,
    /// Add a tool to favorites
    Add {
        /// Tool name
        tool: String,
        /// Pin to a specific version
        #[arg(long)]
        version: Option<String>,
    },
    /// Remove a tool from favorites
    Remove {
        /// Tool name
        tool: String,
    },
}
