/// Event Acknowledgment Layer - Agent D Critical Path
/// Enables agents to acknowledge federation events with vector clock advancement
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use crate::paths::NabiPaths;

/// Federation event with optional vector clock
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FederationEvent {
    pub id: String,
    pub source: String,
    pub severity: String,
    pub message: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_clock: Option<HashMap<String, i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Event acknowledgment with causal tracking
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Acknowledgment {
    pub ack_id: String,
    pub event_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub node_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_clock: Option<HashMap<String, i64>>,
    pub causally_after: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Federation node configuration
#[derive(Debug, Serialize, Deserialize)]
struct FederationConfig {
    node: NodeConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeConfig {
    id: String,
    platform: String,
}

/// Execute acknowledgment command
pub fn execute_ack(
    event_id: &str,
    metadata: Option<serde_json::Value>,
    json_output: bool,
) -> Result<()> {
    // 1. Load event from storage
    let event =
        load_event(event_id).with_context(|| format!("Failed to load event: {}", event_id))?;

    // 2. Load existing acknowledgments for this event
    let existing_acks = load_acknowledgment_log(event_id)?;

    // 3. Get node ID and increment vector clock
    let node_id = get_node_id()?;
    let mut agent_vc = event.vector_clock.clone().unwrap_or_default();
    let current_count = agent_vc.get(&node_id).copied().unwrap_or(0);
    agent_vc.insert(node_id.clone(), current_count + 1);

    // 4. Compute causal ancestors via Python reconciler
    let existing_vcs: Vec<HashMap<String, i64>> = existing_acks
        .iter()
        .filter_map(|ack| ack.vector_clock.clone())
        .collect();

    let ancestor_indices = if !existing_vcs.is_empty() {
        compute_causal_ancestors(&agent_vc, &existing_vcs)?
    } else {
        Vec::new()
    };

    let causally_after: Vec<String> = ancestor_indices
        .iter()
        .map(|&idx| existing_acks[idx].ack_id.clone())
        .collect();

    // 5. Create acknowledgment entry
    let ack = Acknowledgment {
        ack_id: format!("ack-{}", Uuid::new_v4()),
        event_id: event_id.to_string(),
        agent_id: get_agent_id()?,
        session_id: get_session_id()?,
        node_id,
        timestamp: Utc::now().to_rfc3339(),
        vector_clock: Some(agent_vc.clone()),
        causally_after: causally_after.clone(),
        metadata,
    };

    // 6. Atomic append to JSONL acknowledgment log
    let ack_log_path = get_ack_log_path(event_id)?;
    atomic_append_jsonl(&ack_log_path, &ack).context("Failed to write acknowledgment to log")?;

    // 7. Output result
    if json_output {
        println!("{}", serde_json::to_string_pretty(&ack)?);
    } else {
        println!(
            "\x1b[32m✓\x1b[0m Acknowledged event {} as {}",
            event_id, ack.ack_id
        );
        println!("  Vector clock: {:?}", agent_vc);
        if !causally_after.is_empty() {
            println!("  Causally after: {:?}", causally_after);
        }
    }

    Ok(())
}

/// Get node ID from config with fallback chain
fn get_node_id() -> Result<String> {
    // 1. Try federation config file
    let config_path = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("nabi")
        .join("federation.toml");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: FederationConfig = toml::from_str(&content)?;
        return Ok(config.node.id);
    }

    // 2. Try state file
    let state_path = NabiPaths::data_dir()?
        .join("node_id.txt");

    if state_path.exists() {
        let id = fs::read_to_string(&state_path)?;
        return Ok(id.trim().to_string());
    }

    // 3. Generate from hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string()
        .replace('.', "-");
    let node_id = format!("node-{}", hostname);

    // 4. Write back to state file for next time
    fs::create_dir_all(state_path.parent().unwrap())?;
    fs::write(&state_path, &node_id)?;

    Ok(node_id)
}

/// Get agent ID from environment or default
fn get_agent_id() -> Result<String> {
    Ok(env::var("CLAUDE_AGENT_ID").unwrap_or_else(|_| "claude-sonnet".to_string()))
}

/// Get session ID from environment or generate
fn get_session_id() -> Result<String> {
    Ok(env::var("CLAUDE_SESSION_ID").unwrap_or_else(|_| Uuid::new_v4().to_string()))
}

/// Load event from per-file storage
fn load_event(event_id: &str) -> Result<FederationEvent> {
    let state_base = NabiPaths::data_dir()?
        .join("events");

    // Search recent date directories (today through 7 days ago)
    for days_ago in 0..7 {
        let date = Utc::now() - chrono::Duration::days(days_ago);
        let date_str = date.format("%Y-%m-%d").to_string();
        let event_path = state_base
            .join(&date_str)
            .join(format!("{}.json", event_id));

        if event_path.exists() {
            let content = fs::read_to_string(&event_path)?;
            return Ok(serde_json::from_str(&content)?);
        }
    }

    // Fallback: search in main event stream JSONL
    let stream_path = state_base.join("event_stream.jsonl");
    if stream_path.exists() {
        let file = File::open(&stream_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as FederationEvent first
            if let Ok(event) = serde_json::from_str::<FederationEvent>(&line) {
                if event.id == event_id {
                    return Ok(event);
                }
            }
            // Fallback: try to extract id field from generic JSON
            else if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
                    if id == event_id {
                        // Try to convert to FederationEvent format
                        let event = FederationEvent {
                            id: id.to_string(),
                            source: value
                                .get("source")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            severity: value
                                .get("severity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("info")
                                .to_string(),
                            message: value
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            timestamp: value
                                .get("timestamp")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&Utc::now().to_rfc3339())
                                .to_string(),
                            vector_clock: value
                                .get("vector_clock")
                                .and_then(|v| serde_json::from_value(v.clone()).ok()),
                            metadata: value.get("metadata").cloned(),
                        };
                        return Ok(event);
                    }
                }
            }
        }
    }

    anyhow::bail!("Event not found: {}", event_id)
}

/// Load acknowledgment log for an event
pub fn load_acknowledgment_log(event_id: &str) -> Result<Vec<Acknowledgment>> {
    let ack_log_path = match get_ack_log_path(event_id) {
        Ok(path) => path,
        Err(_) => return Ok(Vec::new()),
    };

    if !ack_log_path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&ack_log_path)?;
    let reader = BufReader::new(file);

    let mut acks = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let ack: Acknowledgment = serde_json::from_str(&line)?;
        acks.push(ack);
    }

    Ok(acks)
}

/// Get acknowledgment log path for an event
fn get_ack_log_path(event_id: &str) -> Result<PathBuf> {
    // Use NabiPaths for XDG-compliant path resolution
    let state_base = NabiPaths::data_dir()?
        .join("events");

    // Try to find event file in recent date directories
    for days_ago in 0..7 {
        let date = Utc::now() - chrono::Duration::days(days_ago);
        let date_str = date.format("%Y-%m-%d").to_string();
        let event_path = state_base
            .join(&date_str)
            .join(format!("{}.json", event_id));

        if event_path.exists() {
            return Ok(event_path.with_extension("ack.jsonl"));
        }
    }

    // Default to today if event not found
    let today = Utc::now().format("%Y-%m-%d").to_string();
    Ok(state_base
        .join(&today)
        .join(format!("{}.ack.jsonl", event_id)))
}

/// Atomic append to JSONL file
fn atomic_append_jsonl<T: Serialize>(path: &Path, entry: &T) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create temp file in same directory for atomic rename
    let temp_path = path.with_extension("tmp");

    // Read existing content if file exists
    let mut content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    // Append new entry
    content.push_str(&serde_json::to_string(entry)?);
    content.push('\n');

    // Write to temp file
    fs::write(&temp_path, &content)?;

    // Atomic rename
    fs::rename(&temp_path, path)?;

    Ok(())
}

/// Compute causal ancestors via Python reconciler
fn compute_causal_ancestors(
    target_vc: &HashMap<String, i64>,
    all_vcs: &[HashMap<String, i64>],
) -> Result<Vec<usize>> {
    // Create inline Python bridge script
    let bridge_script = r#"#!/usr/bin/env python3
"""
Python bridge for calling nabia.aether.reconcile from Rust.
Usage: python3 reconciler_bridge.py <target_vc_json> <all_vcs_json>
"""
import sys
import json
from pathlib import Path

# Add nabia platform to path
platform_path = Path.home() / "nabia/platform"
sys.path.insert(0, str(platform_path))
sys.path.insert(0, str(platform_path / "aether/src"))

try:
    from nabia.aether.reconcile import Reconciler
except ImportError as e:
    print(f"Failed to import Reconciler: {e}", file=sys.stderr)
    print(f"Python path: {sys.path}", file=sys.stderr)
    sys.exit(1)

def main():
    if len(sys.argv) != 3:
        print("Usage: reconciler_bridge.py <target_vc_json> <all_vcs_json>", file=sys.stderr)
        sys.exit(1)

    target_vc = json.loads(sys.argv[1])
    all_vcs = json.loads(sys.argv[2])

    reconciler = Reconciler({})
    ancestors = reconciler.compute_causal_ancestors(target_vc, all_vcs)

    print(json.dumps(ancestors))

if __name__ == "__main__":
    main()
"#;

    // Write bridge script to temp file
    let temp_dir = std::env::temp_dir();
    let temp_script = temp_dir.join("reconciler_bridge.py");
    fs::write(&temp_script, bridge_script)?;

    // Call Python with arguments
    let output = Command::new("python3")
        .arg(&temp_script)
        .arg(serde_json::to_string(target_vc)?)
        .arg(serde_json::to_string(all_vcs)?)
        .output()
        .context("Failed to execute Python reconciler")?;

    // Clean up temp script
    let _ = fs::remove_file(&temp_script);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Python reconciler failed: {}", stderr);
    }

    let stdout = String::from_utf8(output.stdout)?;
    let ancestors: Vec<usize> =
        serde_json::from_str(stdout.trim()).context("Failed to parse reconciler output")?;

    Ok(ancestors)
}
