/// Introspection commands for runtime discovery (sessions, windows, panes, agents, etc.)
///
/// These commands expose the current system state for:
/// - Shell completion (dynamic pane/session/window lists)
/// - Federation introspection (agent status, worker discovery)
/// - System querying (available services, ports, etc.)
///
/// Philosophy: "Completions as Discovery" — queries that power both CLI UX and automation.

use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;

#[derive(Subcommand)]
#[command(about = "Introspection commands for runtime discovery and shell completion", long_about =
"Query the runtime for current state to enable dynamic shell completion and federation queries.

These are lightweight introspection endpoints used by:
- Shell completions (nabi list sessions powers tmux pane completion)
- Automation scripts that need canonical entity lists
- Integration with external systems (kubectl, docker, etc.)

All output is plain text, one item per line, suitable for piping to completion systems.")]
pub enum ListCommands {
    /// List all tmux sessions (for completion)
    ///
    /// Queries tmux for active sessions and emits their names.
    /// Used by shell completion to populate session choices.
    ///
    /// Output: One session name per line
    ///
    /// Example: `nabi list sessions | xargs -I {} echo {}`
    #[command(name = "sessions")]
    TmuxSessions {
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
    /// Example: `nabi list windows --session myses`
    #[command(name = "windows")]
    TmuxWindows {
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
    /// Example: `nabi list panes --session myses --window 1`
    #[command(name = "panes")]
    TmuxPanes {
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

pub fn handle_list_commands(cmd: ListCommands) -> Result<()> {
    match cmd {
        ListCommands::TmuxSessions { format, filter } => list_tmux_sessions(&format, filter.as_deref()),
        ListCommands::TmuxWindows { session, format } => list_tmux_windows(&session, &format),
        ListCommands::TmuxPanes { session, window, format } => list_tmux_panes(&session, &window, &format),
    }
}

/// Query tmux for list of sessions
///
/// Format options:
/// - "name": just the session name
/// - "full": session_name (window_count)
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
///
/// Output format:
/// - "id": window_index
/// - "name": window_name
/// - "full": window_index:window_name (default)
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
///
/// Output format:
/// - "id": pane_index (0, 1, 2, ...)
/// - "number": pane_number in 1-based format
/// - "full": session:window.pane (default, for completion)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_format_conversion() {
        // This would test the pane index to pane number conversion
        // In practice, these tests would require running tmux
        // For unit tests, we can verify the parsing logic
    }
}
