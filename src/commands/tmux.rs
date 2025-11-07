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
use chrono;
use clap::{Subcommand, ValueHint};
use serde::{Deserialize, Serialize};
use serde_json;
use std::process::Command;
use std::thread;
use std::time::Duration;

// Timing constants for tmux operations (in milliseconds)
const DELAY_TUI_RECOVERY: u64 = 40;  // Wait after C-c to exit TUI apps
const DELAY_PROMPT_CLEAR: u64 = 20;  // Wait after C-c to clear PS2 prompts
const DELAY_TEXT_TO_ENTER: u64 = 15; // Debounce between text and carriage return

// Supported shell processes (for foreground process detection)
const SUPPORTED_SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "ksh"];

// TUI applications that should receive commands directly (no Ctrl+C recovery)
// These are interactive applications where we want to send text directly
const TUI_DIRECT_SEND: &[&str] = &["claude", "Claude"];

// TUI applications that require Ctrl+C recovery (vim, less, python REPL, etc.)
// These block the shell and need to be exited before sending commands
const TUI_RECOVERY_NEEDED: &[&str] = &["vim", "vi", "nano", "less", "more", "top", "htop", "python", "python3", "ipython", "node", "nodejs", "irb", "pry"];

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
        #[arg(value_name = "PANE", value_hint = ValueHint::Other)]
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
    /// List all tmux sessions (for completion and agent introspection)
    ///
    /// Queries tmux for active sessions with rich metadata.
    /// Provides consistent, predictable output suitable for agent parsing.
    ///
    /// Output formats:
    ///   - json: Structured JSON array with full session metadata (default, for agents)
    ///   - name: One session name per line (for shell completion)
    ///   - full: Human-readable format with window count
    ///
    /// Example: `nabi tmux list sessions | jq '.[] | .name'`
    /// Example: `nabi tmux list sessions --format name | xargs -I {} echo {}`
    #[command(name = "sessions")]
    Sessions {
        /// Output format: json, name, or full (default: json)
        #[arg(long, default_value = "json")]
        format: String,

        /// Show only sessions matching pattern (substring match)
        #[arg(long)]
        filter: Option<String>,
    },

    /// List all tmux windows in a session (for completion and introspection)
    ///
    /// Queries tmux for windows in a specific session.
    /// If no session is provided, uses the current tmux session.
    ///
    /// Output formats:
    ///   - json: Structured JSON array with window metadata (default, for agents)
    ///   - id: Window index numbers only
    ///   - name: Window names only
    ///   - full: window_index:window_name format
    ///
    /// Example: `nabi tmux list windows` (uses current session)
    /// Example: `nabi tmux list windows myses` (specific session)
    /// Example: `nabi tmux list windows --format json`
    #[command(name = "windows")]
    Windows {
        /// Session name to list windows from (defaults to current session if not provided)
        #[arg(value_name = "SESSION")]
        session: Option<String>,

        /// Format for output (default: json)
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// List all tmux panes in a window (for completion and introspection)
    ///
    /// Queries tmux for panes in a specific session:window.
    /// If no session is provided, uses the current tmux session.
    ///
    /// Target format: "session:window" or just "window" (uses current session)
    ///
    /// Output formats:
    ///   - json: Structured JSON array with pane metadata (default, for agents)
    ///   - id: Pane index numbers only
    ///   - number: Pane numbers (1-based)
    ///   - full: session:window.pane format
    ///
    /// Example: `nabi tmux list panes 1` (uses current session, window 1)
    /// Example: `nabi tmux list panes myses:2` (specific session and window)
    #[command(name = "panes")]
    Panes {
        /// Target: "session:window" format or just window number (uses current session)
        /// Examples: "1" (current session, window 1), "myses:2" (session myses, window 2)
        #[arg(value_name = "TARGET")]
        target: String,

        /// Format for output (default: json)
        #[arg(long, default_value = "json")]
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
        ListCommands::Windows { session, format } => {
            let session_name = session.unwrap_or_else(|| get_current_tmux_session().unwrap_or_default());
            list_tmux_windows(&session_name, &format)
        }
        ListCommands::Panes { target, format } => {
            // Parse target: "session:window" or just "window"
            let (session_name, window) = if let Some(colon_pos) = target.find(':') {
                let session = target[..colon_pos].to_string();
                let window = target[colon_pos + 1..].to_string();
                (session, window)
            } else {
                // Just window number, use current session
                let session = get_current_tmux_session()
                    .ok_or_else(|| anyhow::anyhow!("Not in a tmux session. Please specify session:window format (e.g., 'myses:1')"))?;
                (session, target)
            };
            list_tmux_panes(&session_name, &window, &format)
        }
    }
}

/// Get the current tmux session name if running inside tmux
fn get_current_tmux_session() -> Option<String> {
    let output = Command::new("tmux")
        .args(&["display-message", "-p", "#{session_name}"])
        .output()
        .ok()?;

    if output.status.success() {
        let name = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionInfo {
    name: String,
    windows: u32,
    attached: bool,
    created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_claude_tui: Option<bool>,
}

/// Check if a session has Claude TUI panes by querying pane processes
fn check_session_has_claude_tui(session_name: &str) -> bool {
    // Query all panes in all windows of the session for their foreground processes
    // We need to check each window separately since list-panes only works per-window
    let windows_output = Command::new("tmux")
        .args(&[
            "list-windows",
            "-t",
            session_name,
            "-F",
            "#{window_index}",
        ])
        .output();

    if let Ok(windows_result) = windows_output {
        if let Ok(windows_stdout) = String::from_utf8(windows_result.stdout) {
            for window_line in windows_stdout.lines() {
                let window_idx = window_line.trim();
                if window_idx.is_empty() {
                    continue;
                }

                // Check all panes in this window
                let panes_output = Command::new("tmux")
                    .args(&[
                        "list-panes",
                        "-t",
                        &format!("{}:{}", session_name, window_idx),
                        "-F",
                        "#{pane_current_command}",
                    ])
                    .output();

                if let Ok(panes_result) = panes_output {
                    if let Ok(panes_stdout) = String::from_utf8(panes_result.stdout) {
                        for pane_line in panes_stdout.lines() {
                            let process = pane_line.trim();
                            if !process.is_empty() && is_claude_tui(process) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

/// Query tmux for list of sessions with consistent, predictable output
fn list_tmux_sessions(format: &str, filter: Option<&str>) -> Result<()> {
    // Query all session information in one call for consistency
    let output = Command::new("tmux")
        .args(&[
            "list-sessions",
            "-F",
            "#{session_name}|#{session_windows}|#{session_attached}|#{session_created}",
        ])
        .output()
        .context("Failed to query tmux sessions")?;

    if !output.status.success() {
        // No sessions running - return empty result based on format
        match format {
            "json" => println!("[]"),
            _ => {} // Empty output for other formats
        }
        return Ok(());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut sessions: Vec<SessionInfo> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 4 {
            continue; // Skip malformed lines
        }

        let session_name = parts[0].trim();
        let window_count = parts[1].trim().parse::<u32>().unwrap_or(0);
        let attached = parts[2].trim() == "1";
        let created_timestamp = parts[3].trim().parse::<i64>().ok();

        // Apply filter if provided
        if let Some(pattern) = filter {
            if !session_name.contains(pattern) {
                continue;
            }
        }

        let created_str = created_timestamp.map(|ts| {
            // Convert Unix timestamp to ISO 8601 format
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%z").to_string())
                .unwrap_or_else(|| ts.to_string())
        });

        // Check if this session has Claude TUI windows
        // Query the first window's foreground process to detect Claude TUI
        let is_claude = check_session_has_claude_tui(session_name);

        sessions.push(SessionInfo {
            name: session_name.to_string(),
            windows: window_count,
            attached,
            created: created_str,
            is_claude_tui: Some(is_claude),
        });
    }

    // Sort sessions by name for consistent output
    sessions.sort_by(|a, b| a.name.cmp(&b.name));

    match format {
        "json" => {
            // Output as JSON array for structured parsing
            let json = serde_json::to_string_pretty(&sessions)
                .context("Failed to serialize sessions to JSON")?;
            println!("{}", json);
        }
        "full" => {
            // Human-readable format with metadata
            for session in &sessions {
                let attached_str = if session.attached { "attached" } else { "detached" };
                let claude_str = session.is_claude_tui
                    .filter(|&is_claude| is_claude)
                    .map(|_| ", Claude TUI")
                    .unwrap_or_default();
                println!("{} ({} windows, {}{})", session.name, session.windows, attached_str, claude_str);
            }
        }
        "name" | _ => {
            // Simple one-per-line format for shell completion
            for session in &sessions {
                println!("{}", session.name);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct WindowInfo {
    index: u32,
    name: String,
}

/// Query tmux for list of windows in a session
fn list_tmux_windows(session: &str, format: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(&["list-windows", "-t", session, "-F", "#{window_index}:#{window_name}"])
        .output()
        .context(format!("Failed to query windows for session '{}'", session))?;

    if !output.status.success() {
        // Return empty result based on format
        match format {
            "json" => println!("[]"),
            _ => {} // Empty output for other formats
        }
        return Ok(());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut windows: Vec<WindowInfo> = Vec::new();

    for line in stdout.lines() {
        let entry = line.trim();
        if entry.is_empty() {
            continue;
        }

        let parts: Vec<&str> = entry.split(':').collect();
        if parts.len() >= 2 {
            let index = parts[0].trim().parse::<u32>().unwrap_or(0);
            let name = parts[1..].join(":"); // Handle names with colons

            windows.push(WindowInfo {
                index,
                name,
            });
        }
    }

    // Sort by index for consistent output
    windows.sort_by(|a, b| a.index.cmp(&b.index));

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&windows)
                .context("Failed to serialize windows to JSON")?;
            println!("{}", json);
        }
        "id" => {
            for window in &windows {
                println!("{}", window.index);
            }
        }
        "name" => {
            for window in &windows {
                println!("{}", window.name);
            }
        }
        _ => {
            // full format: index:name
            for window in &windows {
                println!("{}:{}", window.index, window.name);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct PaneInfo {
    /// Pane number (1-based, user-friendly - use this in commands)
    pane: u32,
    /// Full target format: session:window.pane (uses 0-based index for tmux)
    target: String,
}

/// Query tmux for list of panes in a window
fn list_tmux_panes(session: &str, window: &str, format: &str) -> Result<()> {
    let target = format!("{}:{}", session, window);

    let output = Command::new("tmux")
        .args(&["list-panes", "-t", &target, "-F", "#{pane_index}"])
        .output()
        .context(format!("Failed to query panes for {}:{}", session, window))?;

    if !output.status.success() {
        // Return empty result based on format
        match format {
            "json" => println!("[]"),
            _ => {} // Empty output for other formats
        }
        return Ok(());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut panes: Vec<PaneInfo> = Vec::new();

    for line in stdout.lines() {
        let pane_index = line.trim();
        if pane_index.is_empty() {
            continue;
        }

        if let Ok(idx) = pane_index.parse::<u32>() {
            // tmux pane_index already respects pane-base-index setting
            // If user has pane-base-index=1, idx will be 1-based
            // We use it directly as the pane number
            panes.push(PaneInfo {
                pane: idx, // Use tmux's pane_index directly (respects pane-base-index)
                target: format!("{}:{}.{}", session, window, idx), // tmux uses this index
            });
        }
    }

    // Sort by pane number for consistent output
    panes.sort_by(|a, b| a.pane.cmp(&b.pane));

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&panes)
                .context("Failed to serialize panes to JSON")?;
            println!("{}", json);
        }
        "id" => {
            // Extract 0-based index from target (session:window.0)
            for pane in &panes {
                if let Some(dot_pos) = pane.target.rfind('.') {
                    if let Ok(idx) = pane.target[dot_pos + 1..].parse::<u32>() {
                        println!("{}", idx);
                    }
                }
            }
        }
        "number" | "pane" => {
            for pane in &panes {
                println!("{}", pane.pane);
            }
        }
        _ => {
            // full format: session:window.pane
            for pane in &panes {
                println!("{}", pane.target);
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

/// Check if a process name indicates a Claude TUI session
///
/// Claude Code TUI sessions often show as version numbers (e.g., "2.0.34")
/// or contain "claude" in the process name. We detect these to avoid
/// sending Ctrl+C which would kill the interactive session.
fn is_claude_tui(process_name: &str) -> bool {
    let lower = process_name.to_lowercase();

    // Check for explicit "claude" in name
    if lower.contains("claude") {
        return true;
    }

    // Check if it looks like a version number (Claude Code shows as version)
    // Pattern: digits.digits.digits (e.g., "2.0.34")
    if process_name.matches('.').count() >= 2 {
        let parts: Vec<&str> = process_name.split('.').collect();
        if parts.len() >= 3 {
            // Check if all parts are numeric (version-like)
            if parts.iter().all(|p| p.parse::<u32>().is_ok()) {
                return true;
            }
        }
    }

    false
}

/// Check if a TUI app requires Ctrl+C recovery before sending commands
fn needs_tui_recovery(process_name: &str) -> bool {
    let lower = process_name.to_lowercase();
    TUI_RECOVERY_NEEDED.iter().any(|&app| lower.contains(app))
}

/// Send message + Enter as reliable operation to target pane
///
/// Implements robust pane state handling:
/// 1. Validate pane format and detect foreground process type
/// 2. For Claude TUI: send commands directly (no recovery needed)
/// 3. For blocking TUI apps: send Ctrl+C to recover to shell
/// 4. For shells: clear continuation state, then send message
/// 5. Send message literally (avoiding interpretation issues)
/// 6. Terminate with carriage return (not "Enter" key name)
///
/// This pattern handles all failure modes:
/// - Claude TUI sessions → send directly (don't kill with C-c)
/// - Blocking TUI apps (vim, less, python REPL, top) → C-c to recover
/// - Shell with unclosed quote/paren/backslash → cleared by C-c
/// - Special characters/backslashes misinterpreted → fixed by -l flag
/// - Enter key timing issues → fixed by using C-m (carriage return)
fn handle_send_prompt(pane: &str, message: &str, delay: u64) -> Result<()> {
    // Validate pane format early
    parse_pane_target(pane)
        .with_context(|| format!("Invalid pane target '{}'", pane))?;

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    // Step 1: Detect foreground process type
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
    let is_claude = is_claude_tui(&fg_process);
    let needs_recovery = needs_tui_recovery(&fg_process);

    // Step 2: Handle different pane states
    if is_claude {
        // Claude TUI: send commands directly, no recovery needed
        // Claude TUI handles input directly at its prompt (>)
    } else if needs_recovery {
        // Blocking TUI app: send Ctrl+C to exit to shell
        let _ = Command::new("tmux")
            .args(&["send-keys", "-t", pane, "C-c"])
            .output();
        thread::sleep(Duration::from_millis(DELAY_TUI_RECOVERY));

        // Clear any continuation prompt
        let _ = Command::new("tmux")
            .args(&["send-keys", "-t", pane, "C-c"])
            .output();
        thread::sleep(Duration::from_millis(DELAY_PROMPT_CLEAR));
    } else if is_shell {
        // Shell: clear any continuation state (PS2 prompt from unclosed quotes/parens/etc)
        let _ = Command::new("tmux")
            .args(&["send-keys", "-t", pane, "C-c"])
            .output();
        thread::sleep(Duration::from_millis(DELAY_PROMPT_CLEAR));
    }
    // For unknown processes, proceed anyway (might be a custom TUI that accepts input)

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

    // Generate descriptive process type for success message
    let process_type = if is_claude {
        "Claude TUI"
    } else if is_shell {
        "shell"
    } else if needs_recovery {
        "TUI (recovered)"
    } else {
        "process"
    };

    println!("✓ Execution complete in pane: {} ({}: {})", pane, process_type, fg_process);
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

    #[test]
    fn test_is_claude_tui_version_number() {
        assert!(is_claude_tui("2.0.34"));
        assert!(is_claude_tui("3.1.2"));
        assert!(is_claude_tui("1.2.3.4"));
    }

    #[test]
    fn test_is_claude_tui_name() {
        assert!(is_claude_tui("claude"));
        assert!(is_claude_tui("Claude"));
        assert!(is_claude_tui("claude-code"));
        assert!(is_claude_tui("my-claude-app"));
    }

    #[test]
    fn test_is_claude_tui_false() {
        assert!(!is_claude_tui("zsh"));
        assert!(!is_claude_tui("vim"));
        assert!(!is_claude_tui("python"));
        assert!(!is_claude_tui("2.0")); // Only 2 parts, not 3+
        assert!(!is_claude_tui("not-a-version"));
    }

    #[test]
    fn test_needs_tui_recovery() {
        assert!(needs_tui_recovery("vim"));
        assert!(needs_tui_recovery("less"));
        assert!(needs_tui_recovery("python"));
        assert!(needs_tui_recovery("python3"));
        assert!(needs_tui_recovery("node"));
        assert!(!needs_tui_recovery("zsh"));
        assert!(!needs_tui_recovery("claude"));
        assert!(!needs_tui_recovery("2.0.34"));
    }

    // Note: Full integration tests would require running tmux,
    // which is not available in all CI/test environments
    // These would test actual tmux command execution and pane interaction
}
