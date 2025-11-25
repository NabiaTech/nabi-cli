/// Acknowledge Event Command - Event Acknowledgment with result_docs
/// Marks events as acknowledged with optional result documentation

use super::common::*;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use crate::paths::NabiPaths;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventAcknowledgment {
    pub ack_id: String,
    pub event_id: String,
    pub status: String,
    pub timestamp: String,
    pub agent_id: String,
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_docs: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_clock: Option<HashMap<String, i64>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AckEventOutput {
    pub ack_id: String,
    pub event_id: String,
    pub status: String,
    pub stored_locally: bool,
    pub stored_in_db: bool,
}

/// Get node ID from config or hostname
fn get_node_id() -> Result<String> {
    // Try federation config first
    let config_path = NabiPaths::config_dir()?
        .join("federation.toml");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: toml::Value = toml::from_str(&content)?;

        if let Some(node_id) = config
            .get("node")
            .and_then(|n| n.get("id"))
            .and_then(|v| v.as_str())
        {
            return Ok(node_id.to_string());
        }
    }

    // Fallback: use hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string()
        .replace('.', "-");
    Ok(format!("node-{}", hostname))
}

/// Get agent ID from environment
fn get_agent_id() -> String {
    std::env::var("CLAUDE_AGENT_ID").unwrap_or_else(|_| "claude-sonnet".to_string())
}

/// Validate acknowledgment status
fn validate_status(status: &str) -> Result<()> {
    let valid = ["acknowledged", "processed", "completed", "failed"];
    if !valid.contains(&status) {
        anyhow::bail!(
            "Invalid status '{}'. Must be one of: {:?}",
            status,
            valid
        );
    }
    Ok(())
}

/// Try to store acknowledgment in SurrealDB (non-blocking, <100ms timeout)
fn try_store_in_db(ack: &EventAcknowledgment) -> bool {
    // TODO: Implement SurrealDB storage with timeout
    // For now, return false to indicate DB storage not available
    // This is non-blocking - failure is acceptable
    false
}

/// Execute ack_event command
pub fn execute(
    event_id: String,
    status: String,
    result_docs: Option<String>,
) -> Result<()> {
    // Validation phase
    validate_status(&status)?;

    // Parse result_docs if provided (should be JSON array)
    let result_docs_value: Option<Vec<serde_json::Value>> = result_docs
        .map(|s| {
            let value: serde_json::Value = serde_json::from_str(&s)?;
            if let Some(array) = value.as_array() {
                Ok(array.clone())
            } else {
                anyhow::bail!("result_docs must be a JSON array")
            }
        })
        .transpose()
        .context("Failed to parse result_docs JSON array")?;

    // Generate acknowledgment ID
    let ack_id = format!("ack-{}", uuid::Uuid::new_v4());
    let timestamp = Utc::now();

    // Get node and agent IDs
    let node_id = get_node_id()?;
    let agent_id = get_agent_id();

    // Create acknowledgment
    let ack = EventAcknowledgment {
        ack_id: ack_id.clone(),
        event_id: event_id.clone(),
        status: status.clone(),
        timestamp: timestamp.to_rfc3339(),
        agent_id: agent_id.clone(),
        node_id: node_id.clone(),
        result_docs: result_docs_value.clone(),
        vector_clock: None, // Kernel will add vector clock if available
    };

    // Phase 1: Store locally (guaranteed, <20ms)
    let ack_path = get_ack_path(&event_id)?;
    let ack_json = serde_json::to_string_pretty(&ack)?;
    fs::write(&ack_path, ack_json)
        .with_context(|| format!("Failed to write ack to {}", ack_path.display()))?;

    // Phase 2: Try to store in SurrealDB (non-blocking, best-effort)
    let stored_in_db = try_store_in_db(&ack);

    // If DB storage fails, write to DLQ for later reconciliation
    if !stored_in_db {
        if let Err(e) = write_to_dlq(&ack, "acks") {
            eprintln!("\x1b[33m⚠\x1b[0m  Failed to write ack to DLQ: {}", e);
        }
    }

    // Phase 3: Return immediately
    let output = AckEventOutput {
        ack_id: ack_id.clone(),
        event_id: event_id.clone(),
        status: "acknowledged".to_string(),
        stored_locally: true,
        stored_in_db,
    };

    // Success output
    println!(
        "\x1b[32m✓\x1b[0m Event acknowledged: {} (Ack ID: {})",
        event_id, ack_id
    );
    println!("  Status: {}", status);
    println!("  Agent: {}", agent_id);
    println!("  Node: {}", node_id);
    if let Some(ref docs) = result_docs_value {
        println!("  Result docs: {} items", docs.len());
    }
    if !stored_in_db {
        println!("  \x1b[33m⚠\x1b[0m  DB storage pending (queued to DLQ)");
    }

    // JSON output for programmatic use
    eprintln!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_status() {
        assert!(validate_status("acknowledged").is_ok());
        assert!(validate_status("processed").is_ok());
        assert!(validate_status("completed").is_ok());
        assert!(validate_status("failed").is_ok());
        assert!(validate_status("invalid").is_err());
    }

    #[test]
    fn test_ack_id_generation() {
        let ack_id = format!("ack-{}", uuid::Uuid::new_v4());
        assert!(ack_id.starts_with("ack-"));
        assert!(ack_id.len() > 10);
    }

    #[test]
    fn test_result_docs_parsing() {
        let json_array = r#"[{"key": "value"}, {"key2": "value2"}]"#;
        let parsed: serde_json::Value = serde_json::from_str(json_array).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }
}
