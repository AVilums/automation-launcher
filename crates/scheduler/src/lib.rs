use chrono::{DateTime, Utc};
use config::LauncherConfig;
use domain::LauncherError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use telemetry::TelemetryManager;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub tool_name: String,
    pub version: Option<String>,
    pub schedule: Schedule,
    pub extra_args: Vec<String>,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Schedule {
    /// Run once at a specific time.
    Once(DateTime<Utc>),
    /// Run every N seconds.
    Interval { seconds: u64 },
    /// Run daily at a specific hour and minute (UTC).
    Daily { hour: u32, minute: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleIndex {
    pub tasks: Vec<ScheduledTask>,
}

pub struct Scheduler {
    index_path: PathBuf,
}

impl Scheduler {
    pub fn new(base_dir: &Path) -> Self {
        Self {
            index_path: base_dir.join("scheduler").join("schedule.json"),
        }
    }

    pub fn init(&self) -> Result<(), LauncherError> {
        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !self.index_path.exists() {
            self.save_index(&ScheduleIndex::default())?;
        }
        Ok(())
    }

    pub fn add_task(&self, task: ScheduledTask) -> Result<(), LauncherError> {
        let mut index = self.load_index()?;
        index.tasks.retain(|t| t.id != task.id);
        info!("Adding scheduled task: {} ({})", task.id, task.tool_name);
        index.tasks.push(task);
        self.save_index(&index)
    }

    pub fn remove_task(&self, task_id: &str) -> Result<(), LauncherError> {
        let mut index = self.load_index()?;
        index.tasks.retain(|t| t.id != task_id);
        info!("Removed scheduled task: {}", task_id);
        self.save_index(&index)
    }

    pub fn list_tasks(&self) -> Result<Vec<ScheduledTask>, LauncherError> {
        let index = self.load_index()?;
        Ok(index.tasks)
    }

    /// Return tasks that are due to run now.
    pub fn due_tasks(&self) -> Result<Vec<ScheduledTask>, LauncherError> {
        let now = Utc::now();
        let index = self.load_index()?;
        let due: Vec<ScheduledTask> = index
            .tasks
            .into_iter()
            .filter(|t| t.enabled && Self::is_due(t, &now))
            .collect();
        Ok(due)
    }

    /// Mark a task as having just run.
    pub fn mark_run(&self, task_id: &str) -> Result<(), LauncherError> {
        let mut index = self.load_index()?;
        if let Some(task) = index.tasks.iter_mut().find(|t| t.id == task_id) {
            task.last_run = Some(Utc::now());
        }
        self.save_index(&index)
    }

    /// Run all due tasks.
    pub async fn run_due_tasks(
        &self,
        config: &LauncherConfig,
        telemetry: &TelemetryManager,
    ) -> Result<usize, LauncherError> {
        let due = self.due_tasks()?;
        let count = due.len();
        for task in due {
            info!("Running scheduled task: {} ({})", task.id, task.tool_name);
            let result = runtime::cmd_run(
                config,
                telemetry,
                &task.tool_name,
                task.version.as_deref(),
                true,
                &task.extra_args,
            )
            .await;

            self.mark_run(&task.id)?;

            if let Err(e) = result {
                tracing::error!("Scheduled task {} failed: {}", task.id, e);
            }
        }
        Ok(count)
    }

    fn is_due(task: &ScheduledTask, now: &DateTime<Utc>) -> bool {
        match &task.schedule {
            Schedule::Once(at) => {
                task.last_run.is_none() && now >= at
            }
            Schedule::Interval { seconds } => {
                match &task.last_run {
                    None => true,
                    Some(last) => {
                        let elapsed = now.signed_duration_since(*last);
                        elapsed.num_seconds() >= *seconds as i64
                    }
                }
            }
            Schedule::Daily { hour, minute } => {
                let now_hour = now.format("%H").to_string().parse::<u32>().unwrap_or(0);
                let now_minute = now.format("%M").to_string().parse::<u32>().unwrap_or(0);
                if now_hour != *hour || now_minute != *minute {
                    return false;
                }
                match &task.last_run {
                    None => true,
                    Some(last) => {
                        let elapsed = now.signed_duration_since(*last);
                        elapsed.num_hours() >= 23
                    }
                }
            }
        }
    }

    fn load_index(&self) -> Result<ScheduleIndex, LauncherError> {
        if !self.index_path.exists() {
            return Ok(ScheduleIndex::default());
        }
        let content = std::fs::read_to_string(&self.index_path)?;
        let index: ScheduleIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    fn save_index(&self, index: &ScheduleIndex) -> Result<(), LauncherError> {
        let content = serde_json::to_string_pretty(index)?;
        std::fs::write(&self.index_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scheduler_init() {
        let tmp = TempDir::new().unwrap();
        let scheduler = Scheduler::new(tmp.path());
        scheduler.init().unwrap();
        assert!(tmp.path().join("scheduler").join("schedule.json").exists());
    }

    #[test]
    fn test_add_and_list_tasks() {
        let tmp = TempDir::new().unwrap();
        let scheduler = Scheduler::new(tmp.path());
        scheduler.init().unwrap();

        let task = ScheduledTask {
            id: "task-1".into(),
            tool_name: "my-tool".into(),
            version: Some("1.0.0".into()),
            schedule: Schedule::Interval { seconds: 60 },
            extra_args: vec![],
            enabled: true,
            last_run: None,
        };

        scheduler.add_task(task).unwrap();
        let tasks = scheduler.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task-1");
    }

    #[test]
    fn test_remove_task() {
        let tmp = TempDir::new().unwrap();
        let scheduler = Scheduler::new(tmp.path());
        scheduler.init().unwrap();

        let task = ScheduledTask {
            id: "task-1".into(),
            tool_name: "my-tool".into(),
            version: None,
            schedule: Schedule::Once(Utc::now()),
            extra_args: vec![],
            enabled: true,
            last_run: None,
        };

        scheduler.add_task(task).unwrap();
        scheduler.remove_task("task-1").unwrap();
        let tasks = scheduler.list_tasks().unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_due_tasks_interval() {
        let tmp = TempDir::new().unwrap();
        let scheduler = Scheduler::new(tmp.path());
        scheduler.init().unwrap();

        let task = ScheduledTask {
            id: "task-1".into(),
            tool_name: "my-tool".into(),
            version: None,
            schedule: Schedule::Interval { seconds: 0 },
            extra_args: vec![],
            enabled: true,
            last_run: None,
        };

        scheduler.add_task(task).unwrap();
        let due = scheduler.due_tasks().unwrap();
        assert_eq!(due.len(), 1);
    }

    #[test]
    fn test_disabled_task_not_due() {
        let tmp = TempDir::new().unwrap();
        let scheduler = Scheduler::new(tmp.path());
        scheduler.init().unwrap();

        let task = ScheduledTask {
            id: "task-1".into(),
            tool_name: "my-tool".into(),
            version: None,
            schedule: Schedule::Interval { seconds: 0 },
            extra_args: vec![],
            enabled: false,
            last_run: None,
        };

        scheduler.add_task(task).unwrap();
        let due = scheduler.due_tasks().unwrap();
        assert!(due.is_empty());
    }
}
