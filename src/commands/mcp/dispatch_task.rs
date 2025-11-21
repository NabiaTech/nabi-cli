/// Dispatch Task Command - Agent Task Signal Creation
/// Creates signal files for agent task dispatch with priority and timeout

use super::common::*;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskSignal {
    pub task_id: String,
    pub agent_id: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub priority: u8,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatcher: Option<String>,
    pub timeout_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_clock: Option<HashMap<String, i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchTaskOutput {
    pub task_id: String,
    pub agent_id: String,
    pub status: String,
    pub signal_path: String,
}

/// Validate agent exists (simple check for now, can be enhanced)
fn validate_agent(agent_id: &str) -> Result<()> {
    // Agent ID validation: lowercase with hyphens/underscores
    if !agent_id
        .chars()
        .all(|c| c.is_lowercase() || c.is_numeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Invalid agent_id '{}'. Must be lowercase with hyphens/underscores only.",
            agent_id
        );
    }

    // Check if agent is registered (optional, based on signal registry)
    // For now, we'll allow any valid agent_id format
    // Future: query signal registry at port 5380 or check file system

    Ok(())
}

/// Validate priority range (1-10)
fn validate_priority(priority: u8) -> Result<()> {
    if !(1..=10).contains(&priority) {
        anyhow::bail!("Priority must be between 1 and 10, got: {}", priority);
    }
    Ok(())
}

/// Execute dispatch_task command
pub fn execute(
    agent: String,
    task_type: String,
    parameters: String,
    priority: Option<u8>,
    timeout_seconds: Option<u64>,
) -> Result<()> {
    // Validation phase
    validate_agent(&agent)?;
    let priority = priority.unwrap_or(5);
    validate_priority(priority)?;
    let timeout = timeout_seconds.unwrap_or(300); // Default 5 minutes

    // Parse parameters JSON
    let parameters_value: serde_json::Value = serde_json::from_str(&parameters)
        .context("Failed to parse parameters JSON")?;

    // Ensure parameters is an object
    if !parameters_value.is_object() {
        anyhow::bail!("Parameters must be a JSON object, not array or primitive");
    }

    // Generate task ID
    let task_id = generate_task_id();
    let timestamp = Utc::now();

    // Get dispatcher from environment or default
    let dispatcher = std::env::var("CLAUDE_AGENT_ID")
        .ok()
        .map(|id| format!("agent:{}", id));

    // Create task signal
    let signal = TaskSignal {
        task_id: task_id.clone(),
        agent_id: agent.clone(),
        task_type: task_type.clone(),
        parameters: parameters_value,
        priority,
        timestamp: timestamp.to_rfc3339(),
        dispatcher,
        timeout_seconds: timeout,
        vector_clock: None, // Kernel will add vector clock if available
        metadata: None,
    };

    // Write signal file
    let signal_path = get_signal_path(&agent, &task_id)?;
    let signal_json = serde_json::to_string_pretty(&signal)?;
    fs::write(&signal_path, signal_json)
        .with_context(|| format!("Failed to write signal file to {}", signal_path.display()))?;

    // Success output
    let output = DispatchTaskOutput {
        task_id: task_id.clone(),
        agent_id: agent.clone(),
        status: "dispatched".to_string(),
        signal_path: signal_path.display().to_string(),
    };

    println!(
        "\x1b[32m✓\x1b[0m Task dispatched to agent '{}': {} (ID: {})",
        agent, task_type, task_id
    );
    println!("  Priority: {}/10", priority);
    println!("  Timeout: {}s", timeout);
    println!("  Signal: {}", signal_path.display());

    // JSON output for programmatic use
    eprintln!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_agent_format() {
        assert!(validate_agent("rust-smith").is_ok());
        assert!(validate_agent("data_forge").is_ok());
        assert!(validate_agent("agent123").is_ok());
        assert!(validate_agent("Invalid-Agent").is_err()); // uppercase not allowed
        assert!(validate_agent("agent!").is_err()); // special chars not allowed
    }

    #[test]
    fn test_validate_priority() {
        assert!(validate_priority(1).is_ok());
        assert!(validate_priority(5).is_ok());
        assert!(validate_priority(10).is_ok());
        assert!(validate_priority(0).is_err());
        assert!(validate_priority(11).is_err());
    }

    #[test]
    fn test_task_id_format() {
        let task_id = generate_task_id();
        assert!(task_id.starts_with("task_"));
        assert!(task_id.contains("T"));
    }
}
