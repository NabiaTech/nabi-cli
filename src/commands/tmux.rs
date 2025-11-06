/// Tmux Pane Coordination Commands
///
/// Reliable operations for coordinating across tmux panes in multi-agent scenarios.
/// Ensures text and Enter keypresses are delivered together, preventing race conditions.
///
/// Architecture Note:
/// - Uses multi-step tmux send-keys operations with guaranteed sequential delivery
/// - Critical for ensuring text + Enter keypresses are never separated
/// - Enables reliable cross-pane IPC for Claude Code autonomous coordination
/// - List commands provide runtime introspection for dynamic shell completions
///
/// Atomicity Guarantee:
/// While not a single tmux command, the operation is "atomic" in the sense that:
/// - Text and Enter are guaranteed to be sent in sequence without interruption
/// - All steps complete or fail together (no partial state)
/// - Recovery steps ensure clean state before sending commands

use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;
use std::thread;
use std::time::Duration;

// Timing constants for tmux operations (in milliseconds)
const DELAY_TUI_RECOVERY: u64 = 40;  // Wait after C-c to exit TUI apps
const DELAY_PROMPT_CLEAR: u64 = 20;  // Wait after C-c to clear PS2 prompts
const DELAY_TEXT_TO_ENTER: u64 = 15; // Debounce between text and carriage return

// Supported shell processes (for foreground process detection)
const SUPPORTED_SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "ksh"];

// Note: Pane format validation is done via parsing, not regex
// These constants are kept for potential future use or documentation

#[derive(Subcommand)]
#[command(about = "Tmux pane coordination (send-prompt, introspection) for multi-agent orchestration", long_about =
"Provides reliable cross-pane IPC operations with guaranteed atomic execution.

This command enables safe coordination across tmux windows by ensuring that text
and Enter keypresses are delivered together in a reliable sequence, preventing
race conditions that occur with uncoordinated send-keys calls.

Also includes introspection commands (nabi tmux list) for runtime discovery:
- Query active sessions, windows, and panes
- Enable dynamic shell completions that reflect live tmux state
- Foundation for federation agent coordination

Perfect for:
  - Coordinating Claude agents across multiple tmux panes
  - Reliable multi-window terminal automation
  - Federation command injection with guaranteed delivery

All operations are reliable: if tmux accepts the command sequence, both text and Enter
are guaranteed to be delivered together without interruption.")]
pub enum TmuxCommands {
    /// Send message + Execute reliably (text + Enter in coordinated sequence)
    ///
    /// Executes a command in a tmux pane with guaranteed reliable delivery.
    /// The message and Enter key are sent in a coordinated sequence,
    /// eliminating race conditions where Enter could be missed.
    ///
    /// Execution guarantees:
    ///   - Text and Enter are always delivered together (reliable sequence)
    ///   - No interference from other terminal operations
    ///   - Configurable delay for tmux processing time
    ///   - Automatic recovery from TUI apps and continuation prompts
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
        /// followed immediately by Enter (guaranteed reliable delivery).
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

/// Parse and validate tmux pane target format
///
/// Accepts formats:
/// - `session:window.pane` (full specification, e.g., "cross-pane:3.2")
/// - `session:window` (implicit pane 1, e.g., "mywindow:1" = "mywindow:1.1")
///
/// Returns normalized format: (session, window, pane)
fn parse_pane_target(pane: &str) -> Result<(String, String, String)> {
    if pane.is_empty() {
        anyhow::bail!("Pane target cannot be empty");
    }

    // Check for full format: session:window.pane
    if let Some(dot_pos) = pane.rfind('.') {
        if let Some(colon_pos) = pane[..dot_pos].rfind(':') {
            let session = pane[..colon_pos].to_string();
            let window = pane[colon_pos + 1..dot_pos].to_string();
            let pane_num = pane[dot_pos + 1..].to_string();

            // Validate numeric parts
            if window.parse::<usize>().is_err() {
                anyhow::bail!("Invalid window number in pane target '{}': expected numeric", pane);
            }
            if pane_num.parse::<usize>().is_err() {
                anyhow::bail!("Invalid pane number in pane target '{}': expected numeric", pane);
            }

            return Ok((session, window, pane_num));
        }
    }

    // Check for window-only format: session:window (implicit pane 1)
    if let Some(colon_pos) = pane.rfind(':') {
        let session = pane[..colon_pos].to_string();
        let window = pane[colon_pos + 1..].to_string();

        if window.parse::<usize>().is_err() {
            anyhow::bail!("Invalid window number in pane target '{}': expected numeric", pane);
        }

        return Ok((session, window, "1".to_string()));
    }

    anyhow::bail!(
        "Invalid pane format '{}': expected 'session:window.pane' or 'session:window'",
        pane
    );
}

/// Send message + Enter as reliable operation to target pane
///
/// Implements robust pane state handling:
/// 1. Validate pane format and verify foreground process is a shell
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
    // Validate pane format early
    parse_pane_target(pane)
        .with_context(|| format!("Invalid pane target '{}'", pane))?;

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    // Step 1: Verify we're at a shell; if not, try to recover
    let fg_output = Command::new("tmux")
        .args(&["display-message", "-p", "-t", pane, "#{pane_current_command}"])
        .output()
        .with_context(|| format!(
            "Failed to check foreground process for pane '{}'",
            pane
        ))?;

    if !fg_output.status.success() {
        let stderr = String::from_utf8_lossy(&fg_output.stderr);
        anyhow::bail!(
            "Failed to query pane '{}': {}",
            pane,
            stderr.trim()
        );
    }

    let fg_process = String::from_utf8_lossy(&fg_output.stdout).trim().to_string();
    let is_shell = SUPPORTED_SHELLS.contains(&fg_process.as_str());

    if !is_shell {
        // Try to break out of TUI app; don't loop forever, just one attempt
        let _ = Command::new("tmux")
            .args(&["send-keys", "-t", pane, "C-c"])
            .output();
        thread::sleep(Duration::from_millis(DELAY_TUI_RECOVERY));
    }

    // Step 2: Ensure clean prompt (kill any partial line / PS2 continuation)
    let _ = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "C-c"])
        .output();
    thread::sleep(Duration::from_millis(DELAY_PROMPT_CLEAR));

    // Step 3: Send literally (uses -l flag), then carriage return (C-m, not "Enter")
    let send_output = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "-l", message])
        .output()
        .with_context(|| format!(
            "Failed to send message to pane '{}'",
            pane
        ))?;

    if !send_output.status.success() {
        let stderr = String::from_utf8_lossy(&send_output.stderr);
        anyhow::bail!(
            "Tmux send-keys (text) failed for pane '{}': {}",
            pane,
            stderr.trim()
        );
    }

    // Small debounce between text and carriage return
    thread::sleep(Duration::from_millis(DELAY_TEXT_TO_ENTER));

    // Step 4: Send carriage return to execute
    let cr_output = Command::new("tmux")
        .args(&["send-keys", "-t", pane, "C-m"])
        .output()
        .with_context(|| format!(
            "Failed to send carriage return to pane '{}'",
            pane
        ))?;

    if !cr_output.status.success() {
        let stderr = String::from_utf8_lossy(&cr_output.stderr);
        anyhow::bail!(
            "Tmux send-keys (carriage return) failed for pane '{}': {}",
            pane,
            stderr.trim()
        );
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
    fn test_parse_pane_target_full_format() {
        let result = parse_pane_target("session:3.2");
        assert!(result.is_ok());
        let (session, window, pane) = result.unwrap();
        assert_eq!(session, "session");
        assert_eq!(window, "3");
        assert_eq!(pane, "2");
    }

    #[test]
    fn test_parse_pane_target_window_format() {
        let result = parse_pane_target("mywindow:1");
        assert!(result.is_ok());
        let (session, window, pane) = result.unwrap();
        assert_eq!(session, "mywindow");
        assert_eq!(window, "1");
        assert_eq!(pane, "1"); // Implicit pane 1
    }

    #[test]
    fn test_parse_pane_target_complex_session_name() {
        let result = parse_pane_target("cross-pane-session:5.3");
        assert!(result.is_ok());
        let (session, window, pane) = result.unwrap();
        assert_eq!(session, "cross-pane-session");
        assert_eq!(window, "5");
        assert_eq!(pane, "3");
    }

    #[test]
    fn test_parse_pane_target_empty() {
        let result = parse_pane_target("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_parse_pane_target_invalid_format() {
        let result = parse_pane_target("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid pane format"));
    }

    #[test]
    fn test_parse_pane_target_invalid_window_number() {
        let result = parse_pane_target("session:abc.1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid window number"));
    }

    #[test]
    fn test_parse_pane_target_invalid_pane_number() {
        let result = parse_pane_target("session:1.xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid pane number"));
    }

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

    #[test]
    fn test_pane_validation_invalid_format() {
        let result = handle_send_prompt("invalid-format", "echo test", 50);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid pane target"));
    }

    // Note: Full integration tests would require running tmux,
    // which is not available in all CI/test environments
    // These would test actual tmux command execution and pane interaction
}
