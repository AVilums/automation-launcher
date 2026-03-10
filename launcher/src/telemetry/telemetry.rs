use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::LauncherError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub tool_name: Option<String>,
    pub tool_version: Option<String>,
    pub status: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    LauncherStartup,
    DownloadStart,
    DownloadComplete,
    DownloadFailure,
    ExecutionStart,
    ExecutionSuccess,
    ExecutionFailure,
}

impl TelemetryEvent {
    pub fn new(event_type: EventType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            tool_name: None,
            tool_version: None,
            status: None,
            details: None,
        }
    }

    pub fn with_tool(mut self, name: &str, version: &str) -> Self {
        self.tool_name = Some(name.to_string());
        self.tool_version = Some(version.to_string());
        self
    }

    pub fn with_status(mut self, status: &str) -> Self {
        self.status = Some(status.to_string());
        self
    }

    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }
}

pub struct TelemetryManager {
    logs_dir: PathBuf,
    enabled: bool,
    remote_endpoint: Option<String>,
    batch: Mutex<Vec<TelemetryEvent>>,
    batch_size: usize,
}

impl TelemetryManager {
    pub fn new(logs_dir: PathBuf, enabled: bool) -> Self {
        Self {
            logs_dir,
            enabled,
            remote_endpoint: None,
            batch: Mutex::new(Vec::new()),
            batch_size: 10,
        }
    }

    pub fn with_remote(mut self, endpoint: Option<String>) -> Self {
        self.remote_endpoint = endpoint;
        self
    }

    #[allow(dead_code)]
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn record(&self, event: &TelemetryEvent) -> Result<(), LauncherError> {
        if !self.enabled {
            return Ok(());
        }

        // Always write to local log
        self.write_local(event)?;

        // Buffer for remote sending
        if self.remote_endpoint.is_some() {
            if let Ok(mut batch) = self.batch.lock() {
                batch.push(event.clone());
            }
        }

        Ok(())
    }

    /// Flush the event batch to the remote endpoint (if configured).
    /// Call this periodically or at shutdown.
    pub async fn flush(&self) -> Result<(), LauncherError> {
        let endpoint = match &self.remote_endpoint {
            Some(ep) => ep.clone(),
            None => return Ok(()),
        };

        let events: Vec<TelemetryEvent> = {
            let mut batch = self
                .batch
                .lock()
                .map_err(|e| LauncherError::Config(format!("Batch lock error: {}", e)))?;
            std::mem::take(&mut *batch)
        };

        if events.is_empty() {
            return Ok(());
        }

        info!("Flushing {} telemetry events to {}", events.len(), endpoint);

        let client = reqwest::Client::new();
        match client.post(&endpoint).json(&events).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Telemetry batch sent successfully");
            }
            Ok(resp) => {
                warn!("Telemetry endpoint returned {}", resp.status());
                // Re-queue events on failure
                if let Ok(mut batch) = self.batch.lock() {
                    batch.extend(events);
                }
            }
            Err(e) => {
                warn!("Failed to send telemetry: {}", e);
                if let Ok(mut batch) = self.batch.lock() {
                    batch.extend(events);
                }
            }
        }

        Ok(())
    }

    /// Check if the batch is ready to flush.
    pub fn should_flush(&self) -> bool {
        self.remote_endpoint.is_some()
            && self
                .batch
                .lock()
                .map(|b| b.len() >= self.batch_size)
                .unwrap_or(false)
    }

    fn write_local(&self, event: &TelemetryEvent) -> Result<(), LauncherError> {
        std::fs::create_dir_all(&self.logs_dir)?;

        let date = event.timestamp.format("%Y-%m-%d").to_string();
        let log_file = self.logs_dir.join(format!("telemetry-{}.jsonl", date));

        let line = serde_json::to_string(event)?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;
        writeln!(file, "{}", line)?;

        info!("Telemetry event recorded: {:?}", event.event_type);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read_events(&self, date: &str) -> Result<Vec<TelemetryEvent>, LauncherError> {
        let log_file = self.logs_dir.join(format!("telemetry-{}.jsonl", date));
        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&log_file)?;
        let events: Vec<TelemetryEvent> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(events)
    }

    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.batch.lock().map(|b| b.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_event() {
        let event = TelemetryEvent::new(EventType::LauncherStartup);
        assert_eq!(event.event_type, EventType::LauncherStartup);
        assert!(event.tool_name.is_none());
    }

    #[test]
    fn test_event_builder() {
        let event = TelemetryEvent::new(EventType::ExecutionStart)
            .with_tool("my-tool", "1.0.0")
            .with_status("running")
            .with_details("Started successfully");

        assert_eq!(event.tool_name, Some("my-tool".into()));
        assert_eq!(event.tool_version, Some("1.0.0".into()));
        assert_eq!(event.status, Some("running".into()));
        assert_eq!(event.details, Some("Started successfully".into()));
    }

    #[test]
    fn test_record_and_read() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), true);

        let event =
            TelemetryEvent::new(EventType::DownloadComplete).with_tool("test-tool", "2.0.0");

        manager.record(&event).unwrap();

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let events = manager.read_events(&date).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::DownloadComplete);
        assert_eq!(events[0].tool_name, Some("test-tool".into()));
    }

    #[test]
    fn test_disabled_telemetry() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), false);

        let event = TelemetryEvent::new(EventType::LauncherStartup);
        manager.record(&event).unwrap();

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let events = manager.read_events(&date).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_multiple_events() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), true);

        for i in 0..5 {
            let event = TelemetryEvent::new(EventType::ExecutionStart)
                .with_tool(&format!("tool-{}", i), "1.0.0");
            manager.record(&event).unwrap();
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let events = manager.read_events(&date).unwrap();
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn test_event_serialization() {
        let event = TelemetryEvent::new(EventType::DownloadFailure)
            .with_tool("broken-tool", "0.1.0")
            .with_details("Connection refused");

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.event_type, EventType::DownloadFailure);
        assert_eq!(deserialized.tool_name, Some("broken-tool".into()));
    }

    #[test]
    fn test_read_nonexistent_date() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), true);

        let events = manager.read_events("1999-01-01").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_batch_accumulation() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), true)
            .with_remote(Some("https://example.com/telemetry".into()))
            .with_batch_size(3);

        for _ in 0..2 {
            manager
                .record(&TelemetryEvent::new(EventType::LauncherStartup))
                .unwrap();
        }

        assert_eq!(manager.pending_count(), 2);
        assert!(!manager.should_flush());

        manager
            .record(&TelemetryEvent::new(EventType::LauncherStartup))
            .unwrap();
        assert_eq!(manager.pending_count(), 3);
        assert!(manager.should_flush());
    }

    #[test]
    fn test_no_batch_without_remote() {
        let tmp = TempDir::new().unwrap();
        let manager = TelemetryManager::new(tmp.path().join("logs"), true);

        manager
            .record(&TelemetryEvent::new(EventType::LauncherStartup))
            .unwrap();
        assert_eq!(manager.pending_count(), 0);
        assert!(!manager.should_flush());
    }
}
