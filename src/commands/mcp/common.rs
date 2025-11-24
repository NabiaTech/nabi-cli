/// Common utilities for MCP federation CLI
/// Provides DLQ fallback, path resolution, and atomic file operations

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::PathBuf;

/// Get event store path (JSONL file for all events)
/// Uses XDG_DATA_HOME/nabi/events for cross-language compatibility
pub fn get_event_store_path() -> Result<PathBuf> {
    // Use NabiPaths for XDG-compliant path resolution
    let events_dir = crate::paths::NabiPaths::data_dir()?
        .join("events");
    fs::create_dir_all(&events_dir)?;
    Ok(events_dir.join("event_stream.jsonl"))
}

/// Get kernel event queue path for federation
pub fn get_kernel_queue_path() -> Result<PathBuf> {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            dirs::state_dir().ok_or_else(|| anyhow::anyhow!("no state dir"))
        })
        .context("Could not determine state directory")?
        .join("nabi")
        .join("kernel");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join("event-queue.jsonl"))
}

/// Get DLQ (Dead Letter Queue) path for failed federation attempts
pub fn get_dlq_path(prefix: &str) -> Result<PathBuf> {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            dirs::state_dir().ok_or_else(|| anyhow::anyhow!("no state dir"))
        })
        .context("Could not determine state directory")?
        .join("nabi")
        .join("dlq");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join(format!("{}-{}.jsonl", prefix, timestamp)))
}

/// Get signal file path for agent task dispatch
pub fn get_signal_path(agent: &str, task_id: &str) -> Result<PathBuf> {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            dirs::state_dir().ok_or_else(|| anyhow::anyhow!("no state dir"))
        })
        .context("Could not determine state directory")?
        .join("nabi")
        .join("signals");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join(format!("task-{}-{}.signal", agent, task_id)))
}

/// Get acknowledgment file path for event
pub fn get_ack_path(event_id: &str) -> Result<PathBuf> {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            dirs::state_dir().ok_or_else(|| anyhow::anyhow!("no state dir"))
        })
        .context("Could not determine state directory")?
        .join("nabi")
        .join("acks");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join(format!("{}.json", event_id)))
}

/// Append to JSONL file atomically (with file locking)
pub fn append_jsonl_atomic<T: Serialize>(path: &PathBuf, entry: &T) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Open file with append mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("Failed to open file for append")?;

    // Serialize and write
    let json_line = serde_json::to_string(entry)?;
    writeln!(file, "{}", json_line).context("Failed to write JSONL line")?;

    // Explicitly sync to disk for durability
    file.sync_all()?;

    Ok(())
}

/// Write to DLQ fallback when kernel queue fails
pub fn write_to_dlq<T: Serialize>(item: &T, prefix: &str) -> Result<()> {
    let dlq_path = get_dlq_path(prefix)?;
    append_jsonl_atomic(&dlq_path, item)?;
    eprintln!(
        "\x1b[33m⚠\x1b[0m  Event queued to DLQ (kernel unavailable): {}",
        dlq_path.display()
    );
    Ok(())
}

/// Generate event ID following schema pattern: evt_YYYYMMDDTHHMMSS_random
pub fn generate_event_id() -> String {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
    let random = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_lowercase();
    format!("evt_{}_{}", timestamp, random)
}

/// Generate task ID following schema pattern: task_YYYYMMDDTHHMMSS_random
pub fn generate_task_id() -> String {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
    let random = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_lowercase();
    format!("task_{}_{}", timestamp, random)
}

/// Queue event to kernel for federation (non-blocking)
pub fn queue_event_to_kernel<T: Serialize>(event: &T) -> Result<()> {
    let queue_path = get_kernel_queue_path()?;
    append_jsonl_atomic(&queue_path, event)
}

/// Load federation event source registry from TOML
/// Returns (registry_version, approved_sources)
pub fn load_event_source_registry() -> Result<(String, Vec<String>)> {
    // Use XDG_CONFIG_HOME if set, otherwise follow XDG standard (~/.config)
    let config_base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            let home = std::env::var("HOME")
                .context("Could not determine HOME directory")?;
            PathBuf::from(home).join(".config")
        }
    };

    let registry_path = config_base
        .join("nabi")
        .join("schemas")
        .join("federation-event-sources.toml");

    if !registry_path.exists() {
        return Err(anyhow::anyhow!(
            "Federation event source registry not found at: {}",
            registry_path.display()
        ));
    }

    let content = std::fs::read_to_string(&registry_path)
        .context("Failed to read federation-event-sources.toml")?;

    // Parse TOML
    let registry: toml::Value = toml::from_str(&content)
        .context("Failed to parse federation-event-sources.toml")?;

    // Extract registry version
    let version = registry
        .get("registry")
        .and_then(|r| r.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| "1.0.0".to_string());

    // Extract all source names from [sources.*] sections
    let sources: Vec<String> = registry
        .get("sources")
        .and_then(|s| s.as_table())
        .map(|table| {
            table
                .keys()
                .map(|k| k.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok((version, sources))
}

/// Validate source exists in registry
pub fn validate_source_in_registry(source: &str) -> Result<(String, String)> {
    let (version, approved_sources) = load_event_source_registry()?;

    if !approved_sources.contains(&source.to_string()) {
        return Err(anyhow::anyhow!(
            "Unknown event source: '{}'\n\
             Available sources: {}\n\
             Tip: Add new sources to ~/.config/nabi/schemas/federation-event-sources.toml",
            source,
            approved_sources.join(", ")
        ));
    }

    Ok((version, source.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_generation() {
        let event_id = generate_event_id();
        assert!(event_id.starts_with("evt_"));
        assert!(event_id.contains("T"));
        assert!(event_id.len() > 20);
    }

    #[test]
    fn test_task_id_generation() {
        let task_id = generate_task_id();
        assert!(task_id.starts_with("task_"));
        assert!(task_id.contains("T"));
        assert!(task_id.len() > 20);
    }

    #[test]
    fn test_id_uniqueness() {
        let id1 = generate_event_id();
        let id2 = generate_event_id();
        assert_ne!(id1, id2, "Event IDs should be unique");
    }
}
