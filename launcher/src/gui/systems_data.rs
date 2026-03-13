use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RunnerStatus {
    Online,
    Busy,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerTask {
    pub name: String,
    pub status: String,
    pub duration: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerNode {
    pub machine_name: String,
    pub os: String,
    pub runner_version: String,
    pub status: RunnerStatus,
    pub current_task: Option<String>,
    pub last_heartbeat_secs: u64,
    pub runtimes: Vec<String>,
    pub recent_tasks: Vec<RunnerTask>,
    pub logs: Vec<String>,
    pub endpoint: String,
}

impl RunnerNode {
    pub fn last_heartbeat_display(&self) -> String {
        if self.last_heartbeat_secs < 60 {
            format!("{}s ago", self.last_heartbeat_secs)
        } else if self.last_heartbeat_secs < 3600 {
            format!("{}m ago", self.last_heartbeat_secs / 60)
        } else {
            format!("{}h ago", self.last_heartbeat_secs / 3600)
        }
    }
}
