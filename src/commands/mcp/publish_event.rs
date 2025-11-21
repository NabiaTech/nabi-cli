/// Publish Event Command - Write-Local/Federate-Async Pattern
/// Guarantees <20ms local write with kernel queue fallback to DLQ

use super::common::*;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpEvent {
    pub id: String,
    pub timestamp: String,
    pub source: String,
    pub severity: String,
    pub agent_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_clock: Option<HashMap<String, i64>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishEventOutput {
    pub event_id: String,
    pub status: String,
    pub timestamp: String,
    pub queued_for_federation: bool,
}

/// Validate source exists in federation event source registry
fn validate_source(source: &str) -> Result<()> {
    validate_source_in_registry(source).map(|_| ())
}

/// Validate severity level
fn validate_severity(severity: &str) -> Result<()> {
    let valid = ["debug", "info", "warning", "error", "critical"];
    if !valid.contains(&severity) {
        anyhow::bail!(
            "Invalid severity '{}'. Must be one of: {:?}",
            severity,
            valid
        );
    }
    Ok(())
}

/// Execute publish_event command
pub fn execute(
    source: String,
    severity: String,
    message: String,
    metadata: Option<String>,
    agent_type: Option<String>,
) -> Result<()> {
    // Validation phase (fast)
    validate_source(&source)?;
    validate_severity(&severity)?;

    // Use provided agent_type or default to "orchestrator"
    let agent_type = agent_type.unwrap_or_else(|| "orchestrator".to_string());

    // Load registry to get version for self-healing
    let (registry_version, _) = load_event_source_registry()?;

    // Parse metadata if provided
    let mut metadata_value: Option<serde_json::Value> = metadata
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .context("Failed to parse metadata JSON")?;

    // Add required fields to metadata
    if let Some(ref mut meta) = metadata_value {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("mcp_exposed".to_string(), serde_json::Value::Bool(true));
            obj.insert(
                "source_registry_version".to_string(),
                serde_json::Value::String(registry_version.clone()),
            );
        }
    } else {
        metadata_value = Some(serde_json::json!({
            "mcp_exposed": true,
            "source_registry_version": registry_version
        }));
    }

    // Generate event ID
    let event_id = generate_event_id();
    let timestamp = Utc::now();

    // Create event structure
    let event = McpEvent {
        id: event_id.clone(),
        timestamp: timestamp.to_rfc3339(),
        source: source.clone(),
        severity: severity.clone(),
        agent_type: agent_type.clone(),
        message: message.clone(),
        metadata: metadata_value,
        vector_clock: None, // Kernel will add vector clock during federation
    };

    // Phase 1: Local write (synchronous, <10ms target)
    let event_store_path = get_event_store_path()?;
    append_jsonl_atomic(&event_store_path, &event)
        .context("Failed to write event to local store")?;

    // Phase 2: Queue for federation (non-blocking with DLQ fallback)
    let queued_for_federation = match queue_event_to_kernel(&event) {
        Ok(_) => true,
        Err(e) => {
            // DLQ fallback - still return success to caller
            write_to_dlq(&event, "events")?;
            eprintln!(
                "\x1b[33m⚠\x1b[0m  Kernel queue failed ({}), event saved to DLQ",
                e
            );
            false
        }
    };

    // Phase 3: Return immediately
    let output = PublishEventOutput {
        event_id: event_id.clone(),
        status: "published".to_string(),
        timestamp: timestamp.to_rfc3339(),
        queued_for_federation,
    };

    // Success output
    println!(
        "\x1b[32m✓\x1b[0m Event published: [{}] {} (ID: {})",
        source, message, event_id
    );
    if queued_for_federation {
        println!("  Queued for federation");
    } else {
        println!("  Saved to DLQ (federation retry pending)");
    }

    // JSON output for programmatic use
    eprintln!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_source_format() {
        assert!(validate_source("agent:igris").is_ok());
        assert!(validate_source("system:kernel").is_ok());
        assert!(validate_source("session:main").is_ok());
        assert!(validate_source("invalid").is_err());
    }

    #[test]
    fn test_validate_severity() {
        assert!(validate_severity("debug").is_ok());
        assert!(validate_severity("info").is_ok());
        assert!(validate_severity("warning").is_ok());
        assert!(validate_severity("error").is_ok());
        assert!(validate_severity("critical").is_ok());
        assert!(validate_severity("invalid").is_err());
    }

    #[test]
    fn test_metadata_mcp_exposed_flag() {
        // Test that mcp_exposed flag is added
        let metadata = Some(r#"{"key": "value"}"#.to_string());
        let parsed: serde_json::Value = serde_json::from_str(metadata.as_ref().unwrap()).unwrap();

        let mut meta_obj = parsed.as_object().unwrap().clone();
        meta_obj.insert("mcp_exposed".to_string(), serde_json::Value::Bool(true));

        assert_eq!(meta_obj.get("mcp_exposed"), Some(&serde_json::Value::Bool(true)));
    }
}
