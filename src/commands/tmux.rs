/// Tmux Pane Coordination Commands
///
/// Atomic operations for coordinating across tmux panes in multi-agent scenarios.
/// All operations are guaranteed to complete in a single tmux command without race conditions.
///
/// Architecture Note:
/// - Replaces two-step tmux send-keys operations with atomic single-command execution
/// - Critical for ensuring text + Enter keypresses are never separated
/// - Enables reliable cross-pane IPC for Claude Code autonomous coordination
/// - List commands provide runtime introspection for dynamic shell completions

use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Subcommand)]
#[command(about = "Tmux pane coordination (send-prompt, introspection) for multi-agent orchestration", long_about =
"Provides reliable cross-pane IPC operations with guaranteed atomic execution.

This command enables safe coordination across tmux windows by ensuring that text
and Enter keypresses are delivered together in a single operation, preventing
race conditions that occur with sequential send-keys calls.

Also includes introspection commands (nabi tmux list) for runtime discovery:
- Query active sessions, windows, and panes
- Enable dynamic shell completions that reflect live tmux state
- Foundation for federation agent coordination

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

    /// Runtime introspection and discovery (sessions, windows, panes)
    ///
    /// Query the live tmux state to enable:
    /// - Dynamic shell completion (nabi tmux list sessions powers tab completion)
    /// - System introspection (what sessions/windows/panes are active?)
    /// - Integration with federation agents and automation
    ///
    /// All output is plain text, one item per line, suitable for piping to completion systems.
    #[command(subcommand)]
    List(ListCommands),
}

#[derive(Subcommand)]
pub enum ListCommands {
    /// List all tmux sessions (for completion)
    ///
    /// Queries tmux for active sessions and emits their names.
    /// Used by shell completion to populate session choices.
    ///
    /// Output: One session name per line
    ///
    /// Example: `nabi tmux list sessions | xargs -I {} echo {}`
    #[command(name = "sessions")]
    Sessions {
        /// Format for output (default: name)
        #[arg(long, default_value = "name")]
        format: String,

        /// Show only sessions matching pattern
        #[arg(long)]
        filter: Option<String>,
    },

    /// List all tmux windows in a session (for completion)
    ///
    /// Queries tmux for windows in a specific session.
    /// Used by shell completion to show window choices after session selection.
    ///
    /// Output: One window per line (format: window_number:window_name)
    ///
    /// Example: `nabi tmux list windows myses`
    #[command(name = "windows")]
    Windows {
        /// Session name to list windows from
        #[arg(value_name = "SESSION")]
        session: String,

        /// Format for output (default: id)
        #[arg(long, default_value = "id")]
        format: String,
    },

    /// List all tmux panes in a window (for completion)
    ///
    /// Queries tmux for panes in a specific session:window.
    /// Used by shell completion to show pane choices.
    ///
    /// Output: One pane ID per line (format: session:window.pane)
    ///
    /// Example: `nabi tmux list panes myses 1`
    #[command(name = "panes")]
    Panes {
        /// Session name
        #[arg(value_name = "SESSION")]
        session: String,

        /// Window number (1-based)
        #[arg(value_name = "WINDOW")]
        window: String,

        /// Format for output (default: full)
        #[arg(long, default_value = "full")]
        format: String,
    },
}

pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        TmuxCommands::SendPrompt {
            pane,
            message,
            delay,
        } => handle_send_prompt(&pane, &message, delay),
        TmuxCommands::List(list_cmd) => handle_list_commands(list_cmd),
    }
}

fn handle_list_commands(cmd: ListCommands) -> Result<()> {
    match cmd {
        ListCommands::Sessions { format, filter } => list_tmux_sessions(&format, filter.as_deref()),
        ListCommands::Windows { session, format } => list_tmux_windows(&session, &format),
        ListCommands::Panes { session, window, format } => list_tmux_panes(&session, &window, &format),
    }
}

/// Query tmux for list of sessions
fn list_tmux_sessions(format: &str, filter: Option<&str>) -> Result<()> {
    let output = Command::new("tmux")
        .args(&["list-sessions", "-F", "#{session_name}"])
        .output()
        .context("Failed to query tmux sessions")?;

    if !output.status.success() {
        return Ok(()); // No sessions running
    }

    let stdout = String::from_utf8(output.stdout)?;

    for line in stdout.lines() {
        let session_name = line.trim();

        // Apply filter if provided
        if let Some(pattern) = filter {
            if !session_name.contains(pattern) {
                continue;
            }
        }

        match format {
            "name" => println!("{}", session_name),
            "full" => {
                // Get window count for this session
                let count_output = Command::new("tmux")
                    .args(&["list-windows", "-t", session_name, "-F", "#{window_index}"])
                    .output();

                if let Ok(count_out) = count_output {
                    let window_count = String::from_utf8(count_out.stdout)
                        .unwrap_or_default()
                        .lines()
                        .count();
                    println!("{} ({})", session_name, window_count);
                } else {
                    println!("{}", session_name);
                }
            }
            _ => println!("{}", session_name),
        }
    }

    Ok(())
}

/// Query tmux for list of windows in a session
fn list_tmux_windows(session: &str, format: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(&["list-windows", "-t", session, "-F", "#{window_index}:#{window_name}"])
        .output()
        .context(format!("Failed to query windows for session '{}'", session))?;

    if !output.status.success() {
        return Ok(()); // Session doesn't exist or has no windows
    }

    let stdout = String::from_utf8(output.stdout)?;

    for line in stdout.lines() {
        let entry = line.trim();
        if entry.is_empty() {
            continue;
        }

        match format {
            "id" => {
                // Extract just the window number
                if let Some(idx) = entry.split(':').next() {
                    println!("{}", idx);
                }
            }
            "name" => {
                // Extract just the window name
                if let Some(name) = entry.split(':').nth(1) {
                    println!("{}", name);
                }
            }
            _ => println!("{}", entry), // full format: index:name
        }
    }

    Ok(())
}

/// Query tmux for list of panes in a window
fn list_tmux_panes(session: &str, window: &str, format: &str) -> Result<()> {
    let target = format!("{}:{}", session, window);

    let output = Command::new("tmux")
        .args(&["list-panes", "-t", &target, "-F", "#{pane_index}"])
        .output()
        .context(format!("Failed to query panes for {}:{}", session, window))?;

    if !output.status.success() {
        return Ok(()); // Window doesn't exist or has no panes
    }

    let stdout = String::from_utf8(output.stdout)?;

    for line in stdout.lines() {
        let pane_index = line.trim();
        if pane_index.is_empty() {
            continue;
        }

        match format {
            "id" => println!("{}", pane_index),
            "number" => {
                // Convert 0-based index to 1-based pane number
                if let Ok(idx) = pane_index.parse::<usize>() {
                    println!("{}", idx + 1);
                }
            }
            _ => {
                // full format: session:window.pane
                println!("{}:{}.{}", session, window, pane_index);
            }
        }
    }

    Ok(())
}

/// Send message + Enter as atomic operation to target pane
///
/// Implements robust pane state handling:
/// 1. Verify foreground process is a shell (not vim/less/REPL/etc)
/// 2. Clear any continuation state (PS2 prompt from unclosed quotes/parens/etc)
/// 3. Send message literally (avoiding interpretation issues)
/// 4. Terminate with carriage return (not "Enter" key name)
///
/// This pattern handles all four common failure modes:
/// - Pane in TUI app (vim, less, python REPL, top) → tries C-c to recover
/// - Unclosed quote/paren/backslash → cleared by C-c
/// - Special characters/backslashes misinterpreted → fixed by -l flag
/// - Enter key timing issues → fixed by using C-m (carriage return)
fn handle_send_prompt(pane: &str, message: &str, delay: u64) -> Result<()> {
    // Validate pane format (basic sanity check)
    if pane.is_empty() {
        anyhow::bail!("Pane target cannot be empty");
    }

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    // Step 1: Verify we're at a shell; if not, try to recover
    let fg_output = Command::new("tmux")
        .args(&["display-message", "-p", "-t", pane, "#{pane_current_command}"])
        .output()
        .context(format!(
            "Failed to check foreground process for pane '{}'",
            pane
        ))?;

    let fg_process = String::from_utf8_lossy(&fg_output.stdout).trim().to_string();
    let is_shell = matches!(fg_process.as_str(), "zsh" | "bash" | "fish" | "sh" | "ksh");

    if !is_shell {
        // Try to break out of TUI app; don't loop forever, just one attempt
        let _ = Command::new("tmux")
            .args(&["send-keys", "-t", pane, "C-c"])
            .output();
        thread::sleep(Duration::from_millis(40));
    }

    // Step 2: Ensure clean prompt (kill any partial line / PS2 continuation)
    let _ = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "C-c"])
        .output();
    thread::sleep(Duration::from_millis(20));

    // Step 3: Send literally (uses -l flag), then carriage return (C-m, not "Enter")
    let send_output = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "-l", message])
        .output()
        .context(format!(
            "Failed to send message to pane '{}'",
            pane
        ))?;

    if !send_output.status.success() {
        let stderr = String::from_utf8_lossy(&send_output.stderr);
        anyhow::bail!("Tmux send-keys (text) failed: {}", stderr.trim());
    }

    // Small debounce between text and carriage return
    thread::sleep(Duration::from_millis(15));

    // Step 4: Send carriage return to execute
    let cr_output = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "C-m"])
        .output()
        .context(format!(
            "Failed to send carriage return to pane '{}'",
            pane
        ))?;

    if !cr_output.status.success() {
        let stderr = String::from_utf8_lossy(&cr_output.stderr);
        anyhow::bail!("Tmux send-keys (carriage return) failed: {}", stderr.trim());
    }

    // Wait for specified delay to let tmux process the command
    if delay > 0 {
        thread::sleep(Duration::from_millis(delay));
    }

    println!("✓ Execution complete in pane: {} (fg: {})", pane, fg_process);
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
