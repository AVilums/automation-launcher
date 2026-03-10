# Telemetry System

## Overview

The launcher generates structured telemetry events for usage tracking and diagnostics.

## Event Structure

Each event contains:
- `id` - Unique event identifier (UUID v4)
- `timestamp` - UTC timestamp
- `event_type` - Category of event
- `tool_name` - Optional tool name
- `tool_version` - Optional tool version
- `status` - Optional status string
- `details` - Optional additional details

## Event Types

- `LauncherStartup` - Application started
- `DownloadStart` - Artifact download initiated
- `DownloadComplete` - Artifact download succeeded
- `DownloadFailure` - Artifact download failed
- `ExecutionStart` - Automation execution started
- `ExecutionSuccess` - Automation completed successfully
- `ExecutionFailure` - Automation execution failed

## Storage

Events are stored as JSON Lines (`.jsonl`) files, one file per day:
```
logs/telemetry-2026-03-10.jsonl
```

## Configuration

Telemetry can be enabled/disabled via configuration:
```json
{
  "telemetry": {
    "enabled": true,
    "remote_endpoint": null
  }
}
```

When disabled, `record()` calls are silently ignored.

## Future Enhancements

- Remote telemetry ingestion
- Analytics dashboards
- Event batching for remote endpoints
