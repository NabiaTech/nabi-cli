/// MCP Federation CLI Commands Module
/// Provides 4 core commands for federation event and task management:
/// - publish_event: Write-local event publishing with kernel queue fallback
/// - dispatch_task: Agent task signal creation
/// - ack_event: Event acknowledgment with result_docs support
/// - stream_events: Fast local event reading with filtering

pub mod ack_event;
pub mod common;
pub mod dispatch_task;
pub mod publish_event;
pub mod stream_events;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum McpCommands {
    /// Publish a new event to the federation event bus
    PublishEvent {
        /// Event source registered in federation-event-sources.toml
        #[arg(long, short = 's')]
        source: String,

        /// Event severity level
        #[arg(long, value_parser = ["debug", "info", "warning", "error", "critical"])]
        severity: String,

        /// Human-readable event message
        #[arg(long, short = 'm')]
        message: String,

        /// Type of agent publishing this event (orchestrator, worker, monitor, system)
        #[arg(long, value_parser = ["orchestrator", "worker", "monitor", "system"], default_value = "orchestrator")]
        agent_type: Option<String>,

        /// Optional metadata as JSON string
        #[arg(long)]
        metadata: Option<String>,
    },

    /// Dispatch a task to an agent
    DispatchTask {
        /// Target agent identifier (e.g., 'rust-smith', 'synthesize')
        #[arg(long, short = 'a')]
        agent: String,

        /// Type of task to execute
        #[arg(long, short = 't')]
        task_type: String,

        /// Task parameters as JSON object
        #[arg(long, short = 'p')]
        parameters: String,

        /// Task priority (1-10, default: 5)
        #[arg(long, default_value = "5")]
        priority: Option<u8>,

        /// Task timeout in seconds (default: 300)
        #[arg(long, default_value = "300")]
        timeout_seconds: Option<u64>,
    },

    /// Acknowledge a federation event
    AckEvent {
        /// Event ID to acknowledge
        #[arg(long)]
        event_id: String,

        /// Acknowledgment status
        #[arg(long, value_parser = ["acknowledged", "processed", "completed", "failed"], default_value = "acknowledged")]
        status: String,

        /// Optional result documentation as JSON array
        #[arg(long)]
        result_docs: Option<String>,
    },

    /// Stream events from the federation event bus
    StreamEvents {
        /// Filter by event source (e.g., 'agent:igris')
        #[arg(long)]
        source: Option<String>,

        /// Filter by severity level
        #[arg(long, value_parser = ["debug", "info", "warning", "error", "critical"])]
        severity: Option<String>,

        /// Filter events after this timestamp (ISO8601 format)
        #[arg(long)]
        after_timestamp: Option<String>,

        /// Maximum number of events to return (default: 50)
        #[arg(long, default_value = "50")]
        limit: Option<usize>,
    },
}

/// Handle MCP commands
pub fn handle_mcp_commands(cmd: McpCommands) -> Result<()> {
    match cmd {
        McpCommands::PublishEvent {
            source,
            severity,
            message,
            agent_type,
            metadata,
        } => publish_event::execute(source, severity, message, metadata, agent_type),

        McpCommands::DispatchTask {
            agent,
            task_type,
            parameters,
            priority,
            timeout_seconds,
        } => dispatch_task::execute(agent, task_type, parameters, priority, timeout_seconds),

        McpCommands::AckEvent {
            event_id,
            status,
            result_docs,
        } => ack_event::execute(event_id, status, result_docs),

        McpCommands::StreamEvents {
            source,
            severity,
            after_timestamp,
            limit,
        } => stream_events::execute(source, severity, after_timestamp, limit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_generation() {
        let id = common::generate_event_id();
        assert!(id.starts_with("evt_"));
    }

    #[test]
    fn test_task_id_generation() {
        let id = common::generate_task_id();
        assert!(id.starts_with("task_"));
    }
}
