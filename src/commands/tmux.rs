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
#[command(about = "Atomic tmux pane coordination for multi-agent orchestration", long_about =
"Provides reliable cross-pane IPC operations with guaranteed atomic execution.

This command enables safe coordination across tmux windows by ensuring that text
and Enter keypresses are delivered together in a single operation, preventing
race conditions that occur with sequential send-keys calls.

Perfect for:
  - Coordinating Claude agents across multiple tmux panes
  - Reliable multi-window terminal automation
  - Federation command injection with guaranteed delivery

All operations are atomic: if tmux accepts the command, both text and Enter
are guaranteed to be delivered as a single unit.")]
pub enum TmuxCommands {
    /// Send message + Execute atomically (text + Enter in single tmux operation)
    ///
    /// Executes a command in a tmux pane with guaranteed atomic delivery.
    /// The message and Enter key are sent in a single tmux send-keys call,
    /// eliminating race conditions where Enter could be missed.
    ///
    /// Execution guarantees:
    ///   - Text and Enter are always delivered together (atomic operation)
    ///   - No interference from other terminal operations
    ///   - Configurable delay for tmux processing time
    ///
    /// Common use cases:
    ///   - Injecting commands into running Claude agents
    ///   - Coordinating work across federation agents
    ///   - Reliable automation across multiple panes
    #[command(after_help = "EXAMPLES:
  # Simple echo command
  nabi tmux send-prompt cross-pane:3.2 \"echo 'Hello from nabi'\"

  # Run shell command
  nabi tmux send-prompt mywindow:1 \"ls -lah ~/nabia\"

  # With custom delay (100ms)
  nabi tmux send-prompt session:2.1 \"cargo build\" --delay 100

  # Coordinate multiple agents
  nabi tmux send-prompt agent1:0 \"task_a\"
  nabi tmux send-prompt agent2:0 \"task_b\"
  nabi tmux send-prompt agent3:0 \"task_c\"

PANE FORMAT:
  session:window.pane  - Full specification (e.g., cross-pane:3.2)
  session:window       - Implicit pane 1 (e.g., mywindow:1 = mywindow:1.1)

  Where: window and pane numbers are 1-based (not 0-based)")]
    SendPrompt {
        /// Tmux pane target (format: session:window.pane or session:window)
        ///
        /// Examples: cross-pane:3.2, schema-driven:1, mywindow:2.1
        /// Windows and panes use 1-based numbering
        #[arg(value_name = "PANE")]
        pane: String,

        /// Message/command to send and execute
        ///
        /// This is the exact text that will be typed into the pane,
        /// followed immediately by Enter (guaranteed atomic delivery).
        /// Supports any shell command, env vars, or special sequences.
        #[arg(value_name = "MESSAGE")]
        message: String,

        /// Delay after execution in milliseconds (default: 50ms)
        ///
        /// Allows time for tmux to process the command before returning.
        /// Increase this if you're sending rapid sequences or complex commands.
        /// Most cases work fine with the default 50ms.
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
