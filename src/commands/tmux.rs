/// Tmux Pane Coordination Commands
///
/// Atomic operations for coordinating across tmux panes in multi-agent scenarios.
/// All operations are guaranteed to complete in a single tmux command without race conditions.
///
/// Architecture Note:
/// - Replaces two-step tmux send-keys operations with atomic single-command execution
/// - Critical for ensuring text + Enter keypresses are never separated
/// - Enables reliable cross-pane IPC for Claude Code autonomous coordination

use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Subcommand)]
pub enum TmuxCommands {
    /// Send message + Execute atomically (text + Enter in single operation)
    ///
    /// Guaranteed atomic execution: message and Enter are sent in one tmux command,
    /// preventing race conditions where Enter gets missed.
    ///
    /// Example: nabi tmux send-prompt cross-pane:3.2 "echo hello world"
    SendPrompt {
        /// Tmux pane target (format: session:window.pane or session:window)
        /// Examples: cross-pane:3.2, schema-driven:1, mywindow:2
        #[arg(value_name = "PANE")]
        pane: String,

        /// Message/command to send and execute
        #[arg(value_name = "MESSAGE")]
        message: String,

        /// Delay after execution in milliseconds (default: 50ms)
        /// Allows time for tmux to process before next operation
        #[arg(long, default_value = "50")]
        delay: u64,
    },
}

pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        TmuxCommands::SendPrompt {
            pane,
            message,
            delay,
        } => handle_send_prompt(&pane, &message, delay),
    }
}

/// Send message + Enter as atomic operation to target pane
///
/// Executes: tmux send-keys -t <pane> <message> Enter
/// The key insight is that both arguments are passed in a single send-keys call,
/// preventing the race condition where Enter could be sent before text is fully processed.
fn handle_send_prompt(pane: &str, message: &str, delay: u64) -> Result<()> {
    // Validate pane format (basic sanity check)
    if pane.is_empty() {
        anyhow::bail!("Pane target cannot be empty");
    }

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    // Execute: tmux send-keys -t <pane> <message> Enter
    let output = Command::new("tmux")
        .args(&["send-keys", "-t", pane, message, "Enter"])
        .output()
        .context(format!(
            "Failed to execute tmux send-keys for pane '{}'",
            pane
        ))?;

    // Check for errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Tmux send-keys failed: {}", stderr.trim());
    }

    // Wait for specified delay to let tmux process the command
    if delay > 0 {
        thread::sleep(Duration::from_millis(delay));
    }

    println!("✓ Execution complete in pane: {}", pane);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_validation_empty() {
        let result = handle_send_prompt("", "echo test", 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_validation_empty() {
        let result = handle_send_prompt("test:1.1", "", 50);
        assert!(result.is_err());
    }

    // Note: Full integration tests would require running tmux,
    // which is not available in all CI/test environments
}
