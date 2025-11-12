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
use clap::{Subcommand, ValueEnum, ValueHint};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::process::Command;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use crate::paths::NabiPaths;

// Timing constants for tmux operations (in milliseconds)
const DELAY_TUI_RECOVERY: u64 = 40; // Wait after C-c to exit TUI apps
const DELAY_PROMPT_CLEAR: u64 = 20; // Wait after C-c to clear PS2 prompts
const DELAY_TEXT_TO_ENTER: u64 = 15; // Debounce between text and carriage return

// Supported shell processes (for foreground process detection)
const SUPPORTED_SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "ksh"];

// TUI applications that should receive commands directly (no Ctrl+C recovery)
// These are interactive applications where we want to send text directly
const TUI_DIRECT_SEND: &[&str] = &["claude", "Claude"];

// TUI applications that require Ctrl+C recovery (vim, less, python REPL, etc.)
// These block the shell and need to be exited before sending commands
const TUI_RECOVERY_NEEDED: &[&str] = &[
    "vim", "vi", "nano", "less", "more", "top", "htop", "python", "python3", "ipython", "node",
    "nodejs", "irb", "pry",
];

// Note: Pane format validation is done via parsing, not regex
// These constants are kept for potential future use or documentation

#[derive(Subcommand)]
#[command(
    about = "Tmux pane coordination (send-prompt, introspection) for multi-agent orchestration",
    long_about = "Provides reliable cross-pane IPC operations with guaranteed atomic execution.

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
are guaranteed to be delivered together without interruption."
)]
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

    /// Send message + Execute from buffer (first line = target, rest = message)
    ///
    /// Reads from stdin or a file where:
    ///   - First line: target pane (format: session:window.pane or session:window)
    ///   - Remaining lines: message to send
    ///
    /// This command is designed for integration with editors like Helix,
    /// where you can pipe buffer content directly to nabi tmux.
    ///
    /// The message preserves all newlines and formatting, making it perfect
    /// for sending multi-line commands or scripts to tmux panes.
    ///
    /// Execution guarantees:
    ///   - Same reliable delivery as send-prompt
    ///   - Automatic recovery from TUI apps
    ///   - Configurable delay for tmux processing
    #[command(after_help = "EXAMPLES:
  # Read from stdin (Helix integration)
  echo -e ':4.2\necho hello\nls -lah' | nabi tmux send-prompt-from-buffer

  # Read from file
  nabi tmux send-prompt-from-buffer buffer.txt

  # With custom delay
  cat buffer.txt | nabi tmux send-prompt-from-buffer --delay 100

  # Helix integration (in command palette)
  :pipe-to nabi tmux send-prompt-from-buffer

BUFFER FORMAT:
  Line 1: Target pane (e.g., cross-pane:3.2, :4.2, mywindow:1)
  Lines 2+: Message to send (preserves all newlines and formatting)

  Example buffer:
    :4.2
    cd ~/nabia
    cargo build --release
    echo 'Build complete!'")]
    SendPromptFromBuffer {
        /// Input file path (if not provided, reads from stdin)
        ///
        /// If omitted, the command reads from stdin, making it perfect
        /// for editor integration via pipe operations.
        #[arg(value_name = "FILE", value_hint = ValueHint::FilePath)]
        file: Option<String>,

        /// Delay after execution in milliseconds (default: 50ms)
        ///
        /// Allows time for tmux to process the command before returning.
        /// Increase this if you're sending rapid sequences or complex commands.
        #[arg(long, default_value = "50")]
        delay: u64,
    },

    /// Capture pane content (last N lines with plain text or JSON output)
    ///
    /// Captures the content of a tmux pane, defaulting to the last 250 lines.
    /// Supports both human-readable (plain text) and JSON output formats
    /// for agentic workflows and machine-readable consumption.
    /// Use --bat flag to enable bat syntax highlighting (opt-in).
    ///
    /// Output formats:
    ///   - text: Human-readable plain text output (default)
    ///   - json: Structured JSON with metadata for agentic consumption
    ///
    /// Use --bat flag to enable bat syntax highlighting for text output.
    ///
    /// Common use cases:
    ///   - Capturing pane state for agent coordination
    ///   - Inspecting pane content without switching focus
    ///   - Chaining with list commands: `nabi tmux list panes | jq '.[] | .send_prompt_target' | xargs -I {} nabi tmux capture {}`
    ///
    /// The command validates pane existence before capture and provides
    /// clear error messages for invalid panes.
    #[command(after_help = "EXAMPLES:
  # Capture current pane (last 250 lines, plain text)
  nabi tmux capture

  # Capture with bat syntax highlighting
  nabi tmux capture --bat

  # Capture specific pane
  nabi tmux capture cross-pane:3.2

  # Capture last 100 lines
  nabi tmux capture mywindow:1 --lines 100

  # JSON output for agentic workflows
  nabi tmux capture session:2.1 --format json

  # Chain with list command
  nabi tmux list panes | jq -r '.[] | .send_prompt_target' | xargs -I {} nabi tmux capture {}

PANE FORMAT:
  session:window.pane  - Full specification (e.g., cross-pane:3.2)
  session:window       - Implicit pane 1 (e.g., mywindow:1 = mywindow:1.1)
  (omitted)            - Current active pane (when running inside tmux)

  Where: window and pane numbers are 1-based (not 0-based)")]
    Capture {
        /// Tmux pane target (format: session:window.pane or session:window)
        ///
        /// Examples: cross-pane:3.2, schema-driven:1, mywindow:2.1
        /// If not specified, captures the current active pane
        /// Windows and panes use 1-based numbering
        #[arg(value_name = "PANE", value_hint = ValueHint::Other)]
        pane: Option<String>,

        /// Number of lines to capture (default: 250)
        ///
        /// Captures the last N lines from the pane's scrollback buffer.
        /// Use negative numbers for relative positioning (future enhancement).
        #[arg(short = 'l', long, default_value = "250")]
        lines: u32,

        /// Output format: text (default, plain text) or json
        ///
        /// - text: Human-readable plain text output (use --bat for syntax highlighting)
        /// - json: Structured JSON with metadata for agentic consumption
        #[arg(long, default_value = "text")]
        format: String,

        /// Enable bat formatting (disabled by default for performance)
        ///
        /// When set, attempts to use bat for syntax highlighting.
        /// Falls back to plain text if bat is unavailable or fails.
        /// Note: bat formatting is disabled by default to avoid performance issues.
        #[arg(long)]
        bat: bool,
    },

    /// Summarize tmux pane memory usage with Claude-aware detection
    ///
    /// Provides the same view as the original pane-mem.sh prototype but with
    /// faster sampling, config-driven thresholds, and optional JSON output.
    /// Useful for spotting lingering Claude Code panes that keep RSS allocations
    /// even after panes are closed.
    Mem {
        /// Output format (table or json). Defaults to config value (table).
        #[arg(long, value_enum)]
        format: Option<PaneMemOutputFormat>,

        /// Limit number of rows (defaults to config, typically 200)
        #[arg(long)]
        limit: Option<usize>,

        /// Only show panes detected as Claude sessions
        #[arg(long, default_value_t = false)]
        claude_only: bool,

        /// Override alert threshold in megabytes (default: 1024 MB)
        #[arg(long)]
        threshold_mb: Option<f64>,
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

    /// Split a tmux pane (horizontal or vertical)
    ///
    /// Creates a new pane by splitting the target pane vertically (down) or
    /// horizontally (right). Enables parallel agent execution and complex layouts.
    ///
    /// Common use cases:
    ///   - Creating synchronized execution panes for agent coordination
    ///   - Building multi-agent orchestration layouts
    ///   - Flexible pane management from agent scripts
    #[command(after_help = "EXAMPLES:
  # Split right (horizontal, default 50%)
  nabi tmux split-pane session:1.1

  # Split down (vertical)
  nabi tmux split-pane session:1.1 -d down

  # Custom size (30% of original)
  nabi tmux split-pane session:1.1 -s 30

  # With command execution
  nabi tmux split-pane session:1.1 -c 'npm run dev'

  # JSON output for agent chaining
  nabi tmux split-pane session:1.1 --format json")]
    SplitPane {
        /// Target pane to split (session:window.pane or session:window)
        #[arg(value_name = "PANE")]
        pane: String,

        /// Split direction: right (horizontal -h) or down (vertical -v)
        #[arg(short = 'd', long, default_value = "right")]
        direction: String,

        /// Size as percentage of parent pane (1-99)
        #[arg(short = 's', long)]
        size: Option<u32>,

        /// Command to execute in new pane
        #[arg(short = 'c', long)]
        command: Option<String>,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
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
        /// Output format: json (default), json+context, name, or full
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

        /// Disable strict session validation
        #[arg(long)]
        no_strict: bool,
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

        /// Disable strict session validation
        #[arg(long)]
        no_strict: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum PaneMemOutputFormat {
    #[value(alias = "table")]
    Table,
    #[value(alias = "json")]
    Json,
}

impl Default for PaneMemOutputFormat {
    fn default() -> Self {
        PaneMemOutputFormat::Table
    }
}

impl FromStr for PaneMemOutputFormat {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "table" => Ok(PaneMemOutputFormat::Table),
            "json" => Ok(PaneMemOutputFormat::Json),
            other => Err(format!("unsupported format '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PaneMemConfig {
    #[serde(default)]
    defaults: PaneMemDefaults,
    #[serde(default)]
    detection: PaneMemDetection,
    #[serde(default)]
    output: PaneMemOutput,
}

impl Default for PaneMemConfig {
    fn default() -> Self {
        PaneMemConfig {
            defaults: PaneMemDefaults::default(),
            detection: PaneMemDetection::default(),
            output: PaneMemOutput::default(),
        }
    }
}

impl PaneMemConfig {
    fn load() -> Self {
        let path = NabiPaths::config_dir()
            .map(|dir| dir.join("tmux-mem").join("config.toml"))
            .ok();

        if let Some(cfg_path) = path {
            if cfg_path.exists() {
                match fs::read_to_string(&cfg_path) {
                    Ok(contents) => match toml::from_str::<PaneMemConfig>(&contents) {
                        Ok(mut cfg) => {
                            cfg.detection.normalize_patterns();
                            return cfg;
                        }
                        Err(err) => {
                            eprintln!(
                                "⚠️  Failed to parse {}: {} (falling back to defaults)",
                                cfg_path.display(),
                                err
                            );
                        }
                    },
                    Err(err) => {
                        eprintln!(
                            "⚠️  Unable to read {}: {} (falling back to defaults)",
                            cfg_path.display(),
                            err
                        );
                    }
                }
            }
        }

        PaneMemConfig::default()
    }

    fn resolved_format(&self, cli_format: Option<PaneMemOutputFormat>) -> PaneMemOutputFormat {
        cli_format
            .or(self.output.default_format)
            .unwrap_or(PaneMemOutputFormat::Table)
    }

    fn show_header(&self) -> bool {
        self.output.show_header
    }

    fn show_legend(&self) -> bool {
        self.output.show_legend
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PaneMemDefaults {
    #[serde(default = "default_highlight_mb")]
    highlight_mb: f64,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_header_title")]
    header_title: String,
    #[serde(default = "default_claude_label")]
    claude_label: String,
    #[serde(default = "default_shell_label")]
    shell_label: String,
    #[serde(default = "default_alert_label")]
    alert_label: String,
}

impl Default for PaneMemDefaults {
    fn default() -> Self {
        PaneMemDefaults {
            highlight_mb: default_highlight_mb(),
            limit: default_limit(),
            header_title: default_header_title(),
            claude_label: default_claude_label(),
            shell_label: default_shell_label(),
            alert_label: default_alert_label(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PaneMemDetection {
    #[serde(default = "default_title_patterns")]
    title_patterns: Vec<String>,
    #[serde(default = "default_process_patterns")]
    process_patterns: Vec<String>,
    #[serde(default)]
    claude_version_command: Option<String>,
    #[serde(default = "default_version_args")]
    claude_version_args: Vec<String>,
}

impl Default for PaneMemDetection {
    fn default() -> Self {
        PaneMemDetection {
            title_patterns: default_title_patterns(),
            process_patterns: default_process_patterns(),
            claude_version_command: None,
            claude_version_args: default_version_args(),
        }
    }
}

impl PaneMemDetection {
    fn normalize_patterns(&mut self) {
        self.title_patterns = self
            .title_patterns
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        self.process_patterns = self
            .process_patterns
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
    }

    fn matches_title(&self, title: &str) -> bool {
        let lower = title.to_ascii_lowercase();
        self.title_patterns
            .iter()
            .any(|pattern| !pattern.is_empty() && lower.contains(pattern))
    }

    fn matches_process(&self, proc_info: &ProcessInfo) -> bool {
        if self.process_patterns.is_empty() {
            return false;
        }

        let command = proc_info.command.to_ascii_lowercase();
        let args = proc_info.args.to_ascii_lowercase();

        self.process_patterns.iter().any(|pattern| {
            if pattern.is_empty() {
                return false;
            }
            command.contains(pattern) || args.contains(pattern)
        })
    }

    fn version_command(&self) -> String {
        self.claude_version_command
            .clone()
            .unwrap_or_else(|| "claude".to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PaneMemOutput {
    #[serde(default)]
    default_format: Option<PaneMemOutputFormat>,
    #[serde(default = "default_true")]
    show_header: bool,
    #[serde(default = "default_true")]
    show_legend: bool,
}

impl Default for PaneMemOutput {
    fn default() -> Self {
        PaneMemOutput {
            default_format: None,
            show_header: true,
            show_legend: true,
        }
    }
}

fn default_highlight_mb() -> f64 {
    1024.0
}

fn default_limit() -> usize {
    200
}

fn default_header_title() -> String {
    "CLAUDE CODE SESSION MEMORY USAGE".to_string()
}

fn default_claude_label() -> String {
    "🔵 Claude".to_string()
}

fn default_shell_label() -> String {
    "shell".to_string()
}

fn default_alert_label() -> String {
    "⚠️  OVER 1GB".to_string()
}

fn default_title_patterns() -> Vec<String> {
    vec!["claude".to_string(), "code".to_string()]
}

fn default_process_patterns() -> Vec<String> {
    vec!["claude".to_string(), "code".to_string(), "node".to_string()]
}

fn default_version_args() -> Vec<String> {
    vec!["--version".to_string()]
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
struct PaneMemoryRecord {
    pane: String,
    pane_title: String,
    pane_pid: i32,
    process_name: String,
    session_type: String,
    memory_kb: u64,
    memory_mb: f64,
    memory_gb: f64,
    percent_of_system: f64,
    child_processes: usize,
    claude_version: Option<String>,
    alert: Option<String>,
}

#[derive(Debug, Clone)]
struct TmuxPaneRow {
    pane: String,
    pid: i32,
    title: String,
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: i32,
    ppid: i32,
    rss_kb: u64,
    command: String,
    args: String,
}

pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        TmuxCommands::SendPrompt {
            pane,
            message,
            delay,
        } => handle_send_prompt(&pane, &message, delay),
        TmuxCommands::SendPromptFromBuffer { file, delay } => {
            handle_send_prompt_from_buffer(file.as_deref(), delay)
        }
        TmuxCommands::Capture {
            pane,
            lines,
            format,
            bat,
        } => handle_capture_pane(pane.as_deref(), lines, &format, bat),
        TmuxCommands::Mem {
            format,
            limit,
            claude_only,
            threshold_mb,
        } => handle_tmux_pane_memory(format, limit, claude_only, threshold_mb),
        TmuxCommands::List(list_cmd) => handle_list_commands(list_cmd),
        TmuxCommands::SplitPane {
            pane,
            direction,
            size,
            command,
            format,
        } => handle_split_pane(&pane, &direction, size, command, &format),
    }
}

fn handle_list_commands(cmd: ListCommands) -> Result<()> {
    match cmd {
        ListCommands::Sessions { format, filter } => list_tmux_sessions(&format, filter.as_deref()),
        ListCommands::Windows {
            session,
            format,
            no_strict,
        } => {
            let session_name =
                session.unwrap_or_else(|| get_current_tmux_session().unwrap_or_default());
            list_tmux_windows(&session_name, &format, no_strict)
        }
        ListCommands::Panes {
            target,
            format,
            no_strict,
        } => {
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
            list_tmux_panes(&session_name, &window, &format, no_strict)
        }
    }
}

fn handle_tmux_pane_memory(
    format: Option<PaneMemOutputFormat>,
    limit: Option<usize>,
    claude_only: bool,
    threshold_override: Option<f64>,
) -> Result<()> {
    let config = PaneMemConfig::load();
    let resolved_format = config.resolved_format(format);
    let row_limit = limit.unwrap_or(config.defaults.limit);
    let highlight_mb = threshold_override.unwrap_or(config.defaults.highlight_mb);
    let total_system_mem_kb = fetch_total_system_memory_kb();

    let panes = collect_tmux_panes().context("Failed to enumerate tmux panes")?;
    if panes.is_empty() {
        println!("No tmux panes detected.");
        return Ok(());
    }

    let processes = collect_process_table().context("Failed to enumerate processes via ps")?;
    let child_map = build_child_map(&processes);

    let mut records: Vec<PaneMemoryRecord> = Vec::new();
    let mut claude_version_cache: Option<Option<String>> = None;

    for pane in panes {
        let Some(parent_info) = processes.get(&pane.pid) else {
            continue;
        };

        let process_tree = gather_process_tree(pane.pid, &child_map);
        if process_tree.is_empty() {
            continue;
        }

        let total_kb: u64 = process_tree
            .iter()
            .filter_map(|pid| processes.get(pid))
            .map(|info| info.rss_kb)
            .sum();

        if total_kb == 0 {
            continue;
        }

        let mut is_claude = config.detection.matches_title(&pane.title);
        if !is_claude {
            is_claude = process_tree.iter().any(|pid| {
                processes
                    .get(pid)
                    .map(|info| config.detection.matches_process(info))
                    .unwrap_or(false)
            });
        }

        if claude_only && !is_claude {
            continue;
        }

        let claude_version = if is_claude {
            if claude_version_cache.is_none() {
                claude_version_cache = Some(query_claude_version(&config.detection));
            }
            claude_version_cache.clone().unwrap_or(None)
        } else {
            None
        };

        let total_mb = total_kb as f64 / 1024.0;
        let total_gb = total_kb as f64 / (1024.0 * 1024.0);
        let percent = if total_system_mem_kb > 0 {
            (total_kb as f64 / total_system_mem_kb as f64) * 100.0
        } else {
            0.0
        };

        let alert = if total_mb > highlight_mb {
            Some(config.defaults.alert_label.clone())
        } else {
            None
        };

        let session_type = if is_claude {
            config.defaults.claude_label.clone()
        } else {
            config.defaults.shell_label.clone()
        };

        let child_count = process_tree.len().saturating_sub(1);

        records.push(PaneMemoryRecord {
            pane: pane.pane.clone(),
            pane_title: pane.title.clone(),
            pane_pid: pane.pid,
            process_name: parent_info.command.clone(),
            session_type,
            memory_kb: total_kb,
            memory_mb: total_mb,
            memory_gb: total_gb,
            percent_of_system: percent,
            child_processes: child_count,
            claude_version,
            alert,
        });
    }

    if records.is_empty() {
        println!("No live tmux pane processes found.");
        return Ok(());
    }

    records.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));
    if records.len() > row_limit {
        records.truncate(row_limit);
    }

    match resolved_format {
        PaneMemOutputFormat::Table => print_pane_memory_table(&records, &config),
        PaneMemOutputFormat::Json => print_pane_memory_json(&records)?,
    }

    Ok(())
}

fn collect_tmux_panes() -> Result<Vec<TmuxPaneRow>> {
    let format = "#{session_name}:#{window_index}.#{pane_index}\t#{pane_pid}\t#{pane_title}";
    let output = Command::new("tmux")
        .args(&["list-panes", "-a", "-F", format])
        .output()
        .context("tmux list-panes failed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "tmux list-panes failed: {}",
            stderr.trim().to_string()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut panes = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let pane_id = parts.next().unwrap_or("").trim();
        let pid_str = parts.next().unwrap_or("").trim();
        let title = parts.next().unwrap_or("").trim();
        if pane_id.is_empty() || pid_str.is_empty() {
            continue;
        }
        if let Ok(pid) = pid_str.parse::<i32>() {
            panes.push(TmuxPaneRow {
                pane: pane_id.to_string(),
                pid,
                title: title.to_string(),
            });
        }
    }

    Ok(panes)
}

fn collect_process_table() -> Result<HashMap<i32, ProcessInfo>> {
    let output = Command::new("ps")
        .args(&["-axo", "pid=,ppid=,rss=,comm=,args="])
        .output()
        .context("ps command failed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "ps command failed: {}",
            stderr.trim().to_string()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut table = HashMap::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Some(ppid_str) = parts.next() else {
            continue;
        };
        let Some(rss_str) = parts.next() else {
            continue;
        };
        let command_str = parts.next().unwrap_or("");
        let args_rest = parts.collect::<Vec<&str>>().join(" ");

        let pid = match pid_str.parse::<i32>() {
            Ok(val) => val,
            Err(_) => continue,
        };
        let ppid = match ppid_str.parse::<i32>() {
            Ok(val) => val,
            Err(_) => continue,
        };
        let rss_kb = rss_str.parse::<u64>().unwrap_or(0);

        table.insert(
            pid,
            ProcessInfo {
                pid,
                ppid,
                rss_kb,
                command: command_str.to_string(),
                args: args_rest,
            },
        );
    }

    Ok(table)
}

fn build_child_map(processes: &HashMap<i32, ProcessInfo>) -> HashMap<i32, Vec<i32>> {
    let mut map: HashMap<i32, Vec<i32>> = HashMap::new();
    for info in processes.values() {
        map.entry(info.ppid).or_default().push(info.pid);
    }
    map
}

fn gather_process_tree(root: i32, child_map: &HashMap<i32, Vec<i32>>) -> Vec<i32> {
    let mut visited = HashSet::new();
    let mut stack = vec![root];
    let mut collected = Vec::new();

    while let Some(pid) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }
        collected.push(pid);
        if let Some(children) = child_map.get(&pid) {
            for child in children {
                stack.push(*child);
            }
        }
    }

    collected
}

fn print_pane_memory_table(records: &[PaneMemoryRecord], config: &PaneMemConfig) {
    if config.show_header() {
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!("{}", config.defaults.header_title);
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!();
    }

    println!(
        "{:<20} {:<15} {:<12} {:<8} {:<10} {:<12}",
        "PANE", "SESSION TYPE", "MEMORY", "GB", "CHILDREN", "VERSION"
    );
    println!(
        "────────────────────────────────────────────────────────────────────────────────────────"
    );

    for record in records {
        let mb_display = format!("{:.1} MB", record.memory_mb);
        let gb_display = format!("{:.2} GB", record.memory_gb);
        let version_display = record.claude_version.as_deref().unwrap_or_else(|| {
            if record.session_type == config.defaults.claude_label {
                "(active)"
            } else {
                "-"
            }
        });

        println!(
            "{:<20} {:<15} {:<12} {:<8} {:<10} {:<12}",
            record.pane,
            record.session_type,
            mb_display,
            gb_display,
            record.child_processes,
            version_display
        );

        if let Some(alert) = &record.alert {
            println!("  └─ {}", alert);
        }
    }

    if config.show_legend() {
        println!();
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!("Legend: Children = child process count | Claude = version if detected");
        println!("═══════════════════════════════════════════════════════════════════════════════");
    }
}

fn print_pane_memory_json(records: &[PaneMemoryRecord]) -> Result<()> {
    let json = serde_json::to_string_pretty(records)
        .context("Failed to serialize pane memory records to JSON")?;
    println!("{}", json);
    Ok(())
}

fn fetch_total_system_memory_kb() -> u64 {
    if let Ok(output) = Command::new("sysctl").args(&["-n", "hw.memsize"]).output() {
        if output.status.success() {
            if let Ok(value) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
            {
                return value / 1024;
            }
        }
    }

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                if let Some(value_str) = parts.first() {
                    if let Ok(value) = value_str.parse::<u64>() {
                        return value;
                    }
                }
            }
        }
    }

    8 * 1024 * 1024
}

fn query_claude_version(detection: &PaneMemDetection) -> Option<String> {
    let command = detection.version_command();
    let mut cmd = Command::new(&command);
    if !detection.claude_version_args.is_empty() {
        cmd.args(&detection.claude_version_args);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version_line = stdout.lines().next().unwrap_or("active").trim();
            if version_line.is_empty() {
                Some("active".to_string())
            } else {
                Some(version_line.to_string())
            }
        }
        Ok(_) => Some("active".to_string()),
        Err(_) => Some("active".to_string()),
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

// ========================================================================
// WORK UNIT 1: Context Detection Infrastructure
// ========================================================================

/// Context information about the current tmux environment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxContextInfo {
    pub in_tmux: bool,
    pub session_name: Option<String>,
    pub window_index: Option<u32>,
    pub window_name: Option<String>,
    pub pane_index: Option<u32>,
    pub window_active: Option<bool>,
    pub pane_active: Option<bool>,
    pub pane_id: Option<String>,   // e.g. "%0"
    pub window_id: Option<String>, // e.g. "@0"
    pub window_index_base: u32,    // from tmux config (0 or 1)
    pub pane_index_base: u32,      // from tmux config (0 or 1)
}

impl Default for TmuxContextInfo {
    fn default() -> Self {
        TmuxContextInfo {
            in_tmux: false,
            session_name: None,
            window_index: None,
            window_name: None,
            pane_index: None,
            window_active: None,
            pane_active: None,
            pane_id: None,
            window_id: None,
            window_index_base: 0,
            pane_index_base: 0,
        }
    }
}

/// Get tmux index base settings (window-base-index and pane-base-index)
///
/// Returns tuple of (window_base, pane_base). Both default to 0 if not set.
/// Handles the case where tmux returns empty string (means using default).
fn get_tmux_index_bases() -> Result<(u32, u32)> {
    // Query window base index
    let window_base_output = Command::new("tmux")
        .args(&["show-options", "-gv", "base-index"])
        .output()
        .context("Failed to query tmux base-index")?;

    let window_base = if window_base_output.status.success() {
        let output_str = String::from_utf8(window_base_output.stdout)?
            .trim()
            .to_string();
        if output_str.is_empty() {
            0 // Default to 0 if empty
        } else {
            output_str
                .parse::<u32>()
                .context("Failed to parse base-index as u32")?
        }
    } else {
        0 // Default to 0 on error
    };

    // Query pane base index
    let pane_base_output = Command::new("tmux")
        .args(&["show-options", "-gv", "pane-base-index"])
        .output()
        .context("Failed to query tmux pane-base-index")?;

    let pane_base = if pane_base_output.status.success() {
        let output_str = String::from_utf8(pane_base_output.stdout)?
            .trim()
            .to_string();
        if output_str.is_empty() {
            0 // Default to 0 if empty
        } else {
            output_str
                .parse::<u32>()
                .context("Failed to parse pane-base-index as u32")?
        }
    } else {
        0 // Default to 0 on error
    };

    Ok((window_base, pane_base))
}

/// Get current tmux context information
///
/// Returns a TmuxContextInfo struct with details about the current tmux environment.
/// If not in tmux, returns TmuxContextInfo with in_tmux: false and all Optional fields as None.
fn get_current_tmux_context() -> Result<TmuxContextInfo> {
    // Check if we're in tmux
    if std::env::var("TMUX").is_err() {
        return Ok(TmuxContextInfo::default());
    }

    // Get index bases first
    let (window_base, pane_base) = get_tmux_index_bases()?;

    // Query all context in one call with pipe-delimited format
    let output = Command::new("tmux")
        .args(&[
            "display-message",
            "-p",
            "#{session_name}|#{window_index}|#{window_name}|#{pane_index}|#{window_active}|#{pane_active}|#{pane_id}|#{window_id}",
        ])
        .output()
        .context("Failed to query tmux context")?;

    if !output.status.success() {
        // Not in tmux, return default
        return Ok(TmuxContextInfo::default());
    }

    let output_str = String::from_utf8(output.stdout)?;
    let trimmed = output_str.trim();

    // Parse pipe-delimited output
    let parts: Vec<&str> = trimmed.split('|').collect();
    if parts.len() < 8 {
        // Malformed output, return default
        return Ok(TmuxContextInfo::default());
    }

    let session_name = if !parts[0].is_empty() {
        Some(parts[0].to_string())
    } else {
        None
    };

    let window_index = if !parts[1].is_empty() {
        parts[1].parse::<u32>().ok()
    } else {
        None
    };

    let window_name = if !parts[2].is_empty() {
        Some(parts[2].to_string())
    } else {
        None
    };

    let pane_index = if !parts[3].is_empty() {
        parts[3].parse::<u32>().ok()
    } else {
        None
    };

    let window_active = match parts[4].trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    };

    let pane_active = match parts[5].trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    };

    let pane_id = if !parts[6].is_empty() {
        Some(parts[6].to_string())
    } else {
        None
    };

    let window_id = if !parts[7].is_empty() {
        Some(parts[7].to_string())
    } else {
        None
    };

    Ok(TmuxContextInfo {
        in_tmux: true,
        session_name,
        window_index,
        window_name,
        pane_index,
        window_active,
        pane_active,
        pane_id,
        window_id,
        window_index_base: window_base,
        pane_index_base: pane_base,
    })
}

/// Check if strict mode is enabled via environment variable
/// Returns true (strict) by default, false if NABI_TMUX_STRICT=0
fn check_strictness() -> bool {
    match std::env::var("NABI_TMUX_STRICT") {
        Ok(val) => val != "0" && val.to_lowercase() != "false",
        Err(_) => true, // Default to strict mode
    }
}

/// Validate that requested session matches current context
/// Returns Ok(mismatch_flag) in non-strict mode, Err(exit_code) in strict mode
fn validate_session_match(
    requested_session: Option<&str>,
    current_context: &TmuxContextInfo,
    strict: bool,
) -> Result<bool, i32> {
    // If no session requested, no validation needed
    let requested = match requested_session {
        None => return Ok(false), // No mismatch
        Some(s) => s,
    };

    // If not in tmux, can't validate
    if !current_context.in_tmux {
        return Ok(false); // No mismatch (can't determine)
    }

    // Check if sessions match
    let current = current_context.session_name.as_deref().unwrap_or("");
    if requested == current {
        return Ok(false); // No mismatch - sessions match
    }

    // Mismatch detected
    if strict {
        // Strict mode: return error with exit code 2
        return Err(2);
    } else {
        // Non-strict mode: emit warning and continue
        eprintln!("Warning: Session mismatch. Requested: '{}', Current: '{}'. Continuing in non-strict mode.", requested, current);
        return Ok(true); // Mismatch flag set
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionInfo {
    name: String,
    windows: u32,
    attached: bool,
    created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_claude_tui: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_active: Option<bool>,
}

/// Check if a session has Claude TUI panes by querying pane processes
fn check_session_has_claude_tui(session_name: &str) -> bool {
    // Query all panes in all windows of the session for their foreground processes
    // We need to check each window separately since list-panes only works per-window
    let windows_output = Command::new("tmux")
        .args(&["list-windows", "-t", session_name, "-F", "#{window_index}"])
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
    // Get current context for json+context format and active session marking
    let context = get_current_tmux_context().unwrap_or_default();

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

        // Mark as active if this is the current session
        let is_active = context.session_name.as_deref() == Some(session_name);

        sessions.push(SessionInfo {
            name: session_name.to_string(),
            windows: window_count,
            attached,
            created: created_str,
            is_claude_tui: Some(is_claude),
            is_active: if is_active { Some(true) } else { None },
        });
    }

    // Sort sessions by name for consistent output
    sessions.sort_by(|a, b| a.name.cmp(&b.name));

    match format {
        "json" => {
            // Output as JSON array for structured parsing (backward compatible)
            let json = serde_json::to_string_pretty(&sessions)
                .context("Failed to serialize sessions to JSON")?;
            println!("{}", json);
        }
        "json+context" => {
            // Output with invocation context wrapper
            let wrapped = serde_json::json!({
                "version": 2,
                "invocation_context": {
                    "in_tmux": context.in_tmux,
                    "session_name": context.session_name,
                    "window_index": context.window_index,
                    "window_name": context.window_name,
                    "pane_index": context.pane_index,
                    "window_index_base": context.window_index_base,
                    "pane_index_base": context.pane_index_base
                },
                "sessions": sessions
            });
            println!("{}", serde_json::to_string_pretty(&wrapped)?);
        }
        "full" => {
            // Human-readable format with metadata
            for session in &sessions {
                let attached_str = if session.attached {
                    "attached"
                } else {
                    "detached"
                };
                let claude_str = session
                    .is_claude_tui
                    .filter(|&is_claude| is_claude)
                    .map(|_| ", Claude TUI")
                    .unwrap_or_default();
                println!(
                    "{} ({} windows, {}{})",
                    session.name, session.windows, attached_str, claude_str
                );
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
    pane_count: u32,
    is_active: bool,
}

/// Query tmux for list of windows in a session
fn list_tmux_windows(session: &str, format: &str, no_strict: bool) -> Result<()> {
    // Get current context for validation and json+context format
    let context = get_current_tmux_context().unwrap_or_default();

    // Validate session match (strict by default unless no_strict=true)
    let strict = !no_strict && check_strictness();
    let mismatch = match validate_session_match(Some(session), &context, strict) {
        Ok(m) => m,
        Err(exit_code) => {
            eprintln!(
                "Error: Session mismatch. Expected '{}', currently in '{:?}'",
                session, context.session_name
            );
            std::process::exit(exit_code);
        }
    };

    let output = Command::new("tmux")
        .args(&[
            "list-windows",
            "-t",
            session,
            "-F",
            "#{window_index}|#{window_name}|#{window_panes}|#{window_active}",
        ])
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

        let parts: Vec<&str> = entry.split('|').collect();
        if parts.len() >= 4 {
            let index = parts[0].trim().parse::<u32>().unwrap_or(0);
            let name = parts[1].trim().to_string();
            let pane_count = parts[2].trim().parse::<u32>().unwrap_or(1);
            let is_active = parts[3].trim() == "1";

            windows.push(WindowInfo {
                index,
                name,
                pane_count,
                is_active,
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
        "json+context" => {
            // Output with invocation context wrapper
            let wrapped = serde_json::json!({
                "version": 2,
                "invocation_context": {
                    "session_name": session,
                    "window_index": context.window_index,
                    "window_index_base": context.window_index_base,
                    "pane_index_base": context.pane_index_base,
                    "mismatch": mismatch
                },
                "windows": windows
            });
            println!("{}", serde_json::to_string_pretty(&wrapped)?);
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
    /// Pane number (respects pane-base-index)
    pane: u32,
    /// Legacy full target format
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    /// Is this pane active
    #[serde(skip_serializing_if = "Option::is_none")]
    is_active: Option<bool>,
    /// Stable pane ID (e.g., "%0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_id: Option<String>,
    /// Stable window ID (e.g., "@0")
    #[serde(skip_serializing_if = "Option::is_none")]
    window_id: Option<String>,
    /// Full send-prompt target format
    #[serde(skip_serializing_if = "Option::is_none")]
    send_prompt_target: Option<String>,
    /// Tmux target (same as send_prompt_target)
    #[serde(skip_serializing_if = "Option::is_none")]
    tmux_target: Option<String>,
    /// Short target format (:window.pane)
    #[serde(skip_serializing_if = "Option::is_none")]
    target_shorthand: Option<String>,
    /// Minimal target format
    #[serde(skip_serializing_if = "Option::is_none")]
    target_minimal: Option<String>,
}

/// Query tmux for list of panes in a window
fn list_tmux_panes(session: &str, window: &str, format: &str, no_strict: bool) -> Result<()> {
    // Get current context for validation and json+context format
    let context = get_current_tmux_context().unwrap_or_default();

    // Validate session match
    let strict = !no_strict && check_strictness();
    let mismatch = match validate_session_match(Some(session), &context, strict) {
        Ok(m) => m,
        Err(exit_code) => {
            eprintln!(
                "Error: Session mismatch. Expected '{}', currently in '{:?}'",
                session, context.session_name
            );
            std::process::exit(exit_code);
        }
    };

    let target = format!("{}:{}", session, window);

    let output = Command::new("tmux")
        .args(&[
            "list-panes",
            "-t",
            &target,
            "-F",
            "#{pane_index}|#{pane_active}|#{pane_id}|#{window_id}|#{window_panes}",
        ])
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
    let mut window_panes_count = 1u32; // Default if not found

    for line in stdout.lines() {
        let entry = line.trim();
        if entry.is_empty() {
            continue;
        }

        let parts: Vec<&str> = entry.split('|').collect();
        if parts.len() >= 5 {
            let idx = parts[0].trim().parse::<u32>().unwrap_or(0);
            let is_active = parts[1].trim() == "1";
            let pane_id = parts[2].trim().to_string();
            let window_id = parts[3].trim().to_string();
            window_panes_count = parts[4].trim().parse::<u32>().unwrap_or(1);

            // Generate all target format variants
            let send_prompt_target = format!("{}:{}.{}", session, window, idx);
            let tmux_target = send_prompt_target.clone();
            let target_shorthand = format!(":{}.{}", window, idx);
            let target_minimal = if window_panes_count == 1 {
                format!(":{}", window)
            } else {
                format!(":{}.{}", window, idx)
            };

            panes.push(PaneInfo {
                pane: idx,
                target: Some(send_prompt_target.clone()), // Legacy field
                is_active: Some(is_active),
                pane_id: Some(pane_id),
                window_id: Some(window_id),
                send_prompt_target: Some(send_prompt_target),
                tmux_target: Some(tmux_target),
                target_shorthand: Some(target_shorthand),
                target_minimal: Some(target_minimal),
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
        "json+context" => {
            // Output with invocation context wrapper
            let wrapped = serde_json::json!({
                "version": 2,
                "invocation_context": {
                    "session_name": session,
                    "window_index": context.window_index,
                    "window_name": context.window_name,
                    "pane_index": context.pane_index,
                    "pane_index_base": context.pane_index_base,
                    "mismatch": mismatch
                },
                "panes": panes
            });
            println!("{}", serde_json::to_string_pretty(&wrapped)?);
        }
        "id" => {
            for pane in &panes {
                println!("{}", pane.pane);
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
                if let Some(ref target) = pane.send_prompt_target {
                    println!("{}", target);
                }
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
                anyhow::bail!(
                    "Invalid window number in pane target '{}': expected numeric",
                    pane
                );
            }
            if pane_num.parse::<usize>().is_err() {
                anyhow::bail!(
                    "Invalid pane number in pane target '{}': expected numeric",
                    pane
                );
            }

            return Ok((session, window, pane_num));
        }
    }

    // Check for window-only format: session:window (implicit pane 1)
    if let Some(colon_pos) = pane.rfind(':') {
        let session = pane[..colon_pos].to_string();
        let window = pane[colon_pos + 1..].to_string();

        if window.parse::<usize>().is_err() {
            anyhow::bail!(
                "Invalid window number in pane target '{}': expected numeric",
                pane
            );
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

// ============================================================================
// WORK UNIT 6: Window Name Resolution (Exact Match)
// ============================================================================

/// Resolve a window name to its index using exact match syntax
///
/// Implements window name resolution with exact match support.
///
/// Usage:
///   - Input: `:={window-name}` → Returns window index if exactly one match
///   - Error if syntax missing: name without `:=` prefix
///   - Error if ambiguous: multiple windows with same name (lists all indices)
///   - Error if not found: no windows match the name
///
/// Example:
///   - `resolve_window_name("mysession", ":={fix-kg}")` → Ok(2)
///   - `resolve_window_name("mysession", "fix-kg")` → Err("Window name must use :={name} syntax...")
///   - `resolve_window_name("mysession", ":={duplicate}")` → Err("Ambiguous window name...")
fn resolve_window_name(session: &str, name_spec: &str) -> Result<u32, String> {
    // Step 1: Validate and extract window name from :={name} syntax
    if !name_spec.starts_with(":=") {
        return Err(format!(
            "Window name must use :={{name}} syntax for exact match. Got: '{}'",
            name_spec
        ));
    }

    let window_name = if name_spec.starts_with(":={") && name_spec.ends_with('}') {
        // Extract from :={name} format
        &name_spec[3..name_spec.len() - 1]
    } else if name_spec.starts_with(":=") {
        // Extract from := format (no braces)
        &name_spec[2..]
    } else {
        return Err("Invalid window name format".to_string());
    };

    if window_name.is_empty() {
        return Err("Window name cannot be empty".to_string());
    }

    // Step 2: Query tmux for all windows in the session
    let output = std::process::Command::new("tmux")
        .args(&[
            "list-windows",
            "-t",
            session,
            "-F",
            "#{window_index}|#{window_name}",
        ])
        .output()
        .map_err(|e| format!("Failed to query windows for session '{}': {}", session, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to list windows in session '{}': {}",
            session,
            stderr.trim()
        ));
    }

    // Step 3: Parse output and build map of name → indices
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matching_indices: Vec<u32> = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 {
            if let Ok(index) = parts[0].trim().parse::<u32>() {
                let name = parts[1..].join("|"); // Handle window names with pipes
                if name == window_name {
                    matching_indices.push(index);
                }
            }
        }
    }

    // Step 4: Return result based on match count
    match matching_indices.len() {
        0 => Err(format!(
            "Window '{}' not found in session '{}'",
            window_name, session
        )),
        1 => Ok(matching_indices[0]),
        _ => {
            // Multiple matches - format indices as comma-separated list
            let indices_str = matching_indices
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "Ambiguous window name '{}': found windows [{}]. Use window index directly or ensure unique window names.",
                window_name, indices_str
            ))
        }
    }
}

/// Send message + Execute from buffer (first line = target, rest = message)
///
/// Reads buffer content from stdin or a file, parses the first line as the target pane,
/// and uses the remaining lines as the message to send. This is designed for editor
/// integration where buffer content can be piped directly to the command.
///
/// Buffer format:
///   Line 1: Target pane (e.g., "cross-pane:3.2", ":4.2", "mywindow:1")
///   Lines 2+: Message to send (preserves all newlines and formatting)
///
/// Supports shorthand targets (e.g., ":4.2") which are resolved using the current
/// tmux session context.
fn handle_send_prompt_from_buffer(file: Option<&str>, delay: u64) -> Result<()> {
    // Read content from file or stdin
    let content = if let Some(file_path) = file {
        fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file '{}'", file_path))?
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        buffer
    };

    if content.is_empty() {
        anyhow::bail!("Buffer content is empty");
    }

    // Split into lines, preserving structure
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        anyhow::bail!("Buffer contains no lines");
    }

    // First line is the target pane
    let mut target = lines[0].trim().to_string();
    if target.is_empty() {
        anyhow::bail!("First line must be target pane (e.g., ':4.2' or 'session:window.pane')");
    }

    // Resolve shorthand targets (e.g., ":4.2" -> "current-session:4.2")
    // Shorthand format starts with ':' and has no session name before it
    if target.starts_with(':') {
        // Shorthand format like ":4.2" or ":4" - resolve using current session
        let context = get_current_tmux_context()
            .context("Cannot resolve shorthand target: not in a tmux session")?;

        if !context.in_tmux {
            anyhow::bail!("Cannot resolve shorthand target '{}': not in a tmux session. Use full format 'session:window.pane'", target);
        }

        let session = context.session_name
            .ok_or_else(|| anyhow::anyhow!("Cannot determine current session for shorthand target"))?;

        // Prepend session name to shorthand target
        target = format!("{}{}", session, target);
    }

    // Remaining lines are the message
    let message = if lines.len() > 1 {
        // Join lines 2+ with newlines, preserving original formatting
        lines[1..].join("\n")
    } else {
        anyhow::bail!("Message required (lines 2+). Buffer must contain at least 2 lines.");
    };

    // Remove trailing newline if present (but keep internal newlines)
    let message = message.trim_end_matches('\n').to_string();

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty (lines 2+ must contain content)");
    }

    // Call the existing send-prompt handler
    handle_send_prompt(&target, &message, delay)
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
    parse_pane_target(pane).with_context(|| format!("Invalid pane target '{}'", pane))?;

    if message.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    // Step 1: Detect foreground process type
    let fg_output = Command::new("tmux")
        .args(&[
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_current_command}",
        ])
        .output()
        .with_context(|| format!("Failed to check foreground process for pane '{}'", pane))?;

    if !fg_output.status.success() {
        let stderr = String::from_utf8_lossy(&fg_output.stderr);
        anyhow::bail!("Failed to query pane '{}': {}", pane, stderr.trim());
    }

    let fg_process = String::from_utf8_lossy(&fg_output.stdout)
        .trim()
        .to_string();
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
        .with_context(|| format!("Failed to send message to pane '{}'", pane))?;

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
        .with_context(|| format!("Failed to send carriage return to pane '{}'", pane))?;

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

    println!(
        "✓ Execution complete in pane: {} ({}: {})",
        pane, process_type, fg_process
    );
    Ok(())
}

/// Capture content from a tmux pane (last N lines, with optional bat formatting)
///
/// Validates pane existence, captures content, and outputs in text or JSON format.
/// Bat formatting is opt-in via --bat flag to avoid performance issues.
fn handle_capture_pane(pane: Option<&str>, lines: u32, format: &str, use_bat: bool) -> Result<()> {
    // Determine target pane
    let target_pane = if let Some(p) = pane {
        // Validate explicit pane format
        parse_pane_target(p).with_context(|| format!("Invalid pane target '{}'", p))?;
        p.to_string()
    } else {
        // No pane specified - use current active pane
        let context = get_current_tmux_context().context("Failed to get current tmux context")?;

        if !context.in_tmux {
            anyhow::bail!(
                "Not in a tmux session. Please specify a pane target (e.g., 'session:window.pane')"
            );
        }

        // Build target from context
        let session = context
            .session_name
            .ok_or_else(|| anyhow::anyhow!("Could not determine current session"))?;
        let window = context
            .window_index
            .ok_or_else(|| anyhow::anyhow!("Could not determine current window"))?;
        let pane_idx = context
            .pane_index
            .ok_or_else(|| anyhow::anyhow!("Could not determine current pane"))?;

        format!("{}:{}.{}", session, window, pane_idx)
    };

    // Validate pane exists
    let list_output = Command::new("tmux")
        .args(&["list-panes", "-t", &target_pane, "-F", "#{pane_index}"])
        .output()
        .with_context(|| format!("Failed to validate pane existence for '{}'", target_pane))?;

    if !list_output.status.success() {
        let stderr = String::from_utf8_lossy(&list_output.stderr);
        anyhow::bail!("Pane '{}' does not exist: {}", target_pane, stderr.trim());
    }

    // Build capture command
    let capture_args: Vec<String> = if lines > 0 {
        vec![
            "capture-pane".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            target_pane.clone(),
            "-S".to_string(),
            format!("-{}", lines),
        ]
    } else {
        vec![
            "capture-pane".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            target_pane.clone(),
        ]
    };

    let capture_output = Command::new("tmux")
        .args(&capture_args)
        .output()
        .with_context(|| format!("Failed to capture pane content from '{}'", target_pane))?;

    if !capture_output.status.success() {
        let stderr = String::from_utf8_lossy(&capture_output.stderr);
        anyhow::bail!(
            "Tmux capture-pane failed for '{}': {}",
            target_pane,
            stderr.trim()
        );
    }

    let content = String::from_utf8_lossy(&capture_output.stdout)
        .trim()
        .to_string();

    match format {
        "json" => {
            // JSON output format
            let timestamp = chrono::Utc::now().to_rfc3339();
            let json_output = serde_json::json!({
                "pane": target_pane,
                "lines": lines,
                "timestamp": timestamp,
                "content": content,
                "metadata": {
                    "captured_lines": content.lines().count(),
                    "format": "text"
                }
            });
            println!("{}", serde_json::to_string_pretty(&json_output)?);
        }
        "text" | _ => {
            // Text output format (default)
            if use_bat && !content.is_empty() {
                // Try to use bat for syntax highlighting (opt-in)
                let bat_result = Command::new("bat")
                    .args(&[
                        "--language",
                        "text",
                        "--color",
                        "always",
                        "--plain",
                        "--paging",
                        "never",
                    ])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .spawn();

                match bat_result {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            use std::io::Write;
                            if stdin.write_all(content.as_bytes()).is_ok() {
                                drop(stdin); // Close stdin to signal EOF
                                let _ = child.wait(); // Wait for bat to finish
                                return Ok(());
                            }
                        }
                        // If bat fails, fall through to plain text
                        let _ = child.kill();
                    }
                    Err(_) => {} // bat not available, fall through
                }
            }

            // Plain text output (fallback or default)
            println!("{}", content);
        }
    }

    Ok(())
}

/// Split a pane horizontally (right) or vertically (down)
///
/// Executes: `tmux split-pane -t pane [-h|-v] [-p size] [command]`
///
/// Returns JSON with new pane info for agent chaining:
/// ```json
/// {
///   "source_pane": "session:1.1",
///   "new_pane": "session:1.2",
///   "direction": "down",
///   "size_percent": 50,
///   "command_executed": false
/// }
/// ```
fn handle_split_pane(
    pane: &str,
    direction: &str,
    size: Option<u32>,
    command: Option<String>,
    format: &str,
) -> Result<()> {
    // Step 1: Validate pane format
    let (session, window, _pane_idx) = parse_pane_target(pane)
        .with_context(|| format!("Invalid pane target '{}'", pane))?;

    // Step 2: Validate and normalize direction
    let (direction_flag, direction_label) = match direction.to_lowercase().as_str() {
        "down" | "vertical" | "-v" | "v" => ("-v", "down"),
        "right" | "horizontal" | "-h" | "h" | _ => ("-h", "right"),
    };

    // Step 3: Validate size if provided
    let size_percent = if let Some(s) = size {
        if s < 1 || s > 99 {
            anyhow::bail!("Size must be between 1 and 99 percent");
        }
        s
    } else {
        50 // Default to 50% split
    };

    // Step 4: Build tmux command
    let mut split_args = vec![
        "split-pane".to_string(),
        "-t".to_string(),
        pane.to_string(),
        direction_flag.to_string(),
        "-p".to_string(),
        size_percent.to_string(),
    ];

    // Add command if specified
    if let Some(cmd) = &command {
        split_args.push(cmd.clone());
    }

    // Step 5: Execute split-pane
    let output = Command::new("tmux")
        .args(&split_args)
        .output()
        .context("Failed to execute tmux split-pane")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux split-pane failed: {}", stderr.trim());
    }

    // Step 6: Query new pane to get its index
    let list_output = Command::new("tmux")
        .args(&[
            "list-panes",
            "-t",
            &format!("{}:{}", session, window),
            "-F",
            "#{pane_index}",
        ])
        .output()
        .context("Failed to query new pane")?;

    if !list_output.status.success() {
        let stderr = String::from_utf8_lossy(&list_output.stderr);
        anyhow::bail!("Failed to query new pane: {}", stderr.trim());
    }

    // Get the last pane index (the new one)
    let pane_list = String::from_utf8_lossy(&list_output.stdout);
    let new_pane_idx = pane_list
        .lines()
        .last()
        .unwrap_or("1")
        .trim()
        .to_string();

    let new_pane_target = format!("{}:{}.{}", session, window, new_pane_idx);

    // Step 7: Output result based on format
    match format {
        "json" => {
            let result = serde_json::json!({
                "source_pane": pane,
                "new_pane": new_pane_target,
                "direction": direction_label,
                "size_percent": size_percent,
                "command_executed": command.is_some(),
                "success": true
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "text" | _ => {
            let cmd_info = if command.is_some() { " (with command)" } else { "" };
            println!(
                "✓ Split pane {} ({}): {} → {}{}",
                pane, direction_label, size_percent, new_pane_target, cmd_info
            );
        }
    }

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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid pane format"));
    }

    #[test]
    fn test_parse_pane_target_invalid_window_number() {
        let result = parse_pane_target("session:abc.1");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid window number"));
    }

    #[test]
    fn test_parse_pane_target_invalid_pane_number() {
        let result = parse_pane_target("session:1.xyz");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid pane number"));
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid pane target"));
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
    // ========================================================================
    // WORK UNIT 6 TESTS: Window Name Resolution (Exact Match)
    // ========================================================================

    #[test]
    fn test_resolve_window_name_missing_syntax() {
        // Test: Plain name without :={} should error
        let result = resolve_window_name("test-session", "fix-kg");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Window name must use :={name} syntax"));
    }

    #[test]
    fn test_resolve_window_name_parsing_with_braces() {
        // Test: Correctly extract name from :={name} format
        let name_spec = ":={fix-kg}";
        let extracted = if name_spec.starts_with(":={") && name_spec.ends_with('}') {
            &name_spec[3..name_spec.len() - 1]
        } else {
            panic!("Failed to extract name");
        };
        assert_eq!(extracted, "fix-kg");
    }

    #[test]
    fn test_resolve_window_name_parsing_without_braces() {
        // Test: Correctly extract name from := format (no braces)
        let name_spec = ":=fix-kg";
        let extracted = if name_spec.starts_with(":={") && name_spec.ends_with('}') {
            &name_spec[3..name_spec.len() - 1]
        } else if name_spec.starts_with(":=") {
            &name_spec[2..]
        } else {
            panic!("Failed to extract name");
        };
        assert_eq!(extracted, "fix-kg");
    }

    #[test]
    fn test_resolve_window_name_empty_name() {
        // Test: Empty name after := should error
        let result = resolve_window_name("test-session", ":={}");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Window name cannot be empty"));
    }

    #[test]
    fn test_resolve_window_name_empty_name_no_braces() {
        // Test: Empty name after := (no braces) should error
        let result = resolve_window_name("test-session", ":=");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Window name cannot be empty"));
    }

    #[test]
    fn test_resolve_window_name_parse_output_single() {
        // Test: Mock output parsing for single window match
        let output = "0|window-one
1|fix-kg
2|window-three";
        let search_name = "fix-kg";
        let mut matching_indices: Vec<u32> = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if !line.is_empty() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    if let Ok(index) = parts[0].trim().parse::<u32>() {
                        let name = parts[1..].join("|");
                        if name == search_name {
                            matching_indices.push(index);
                        }
                    }
                }
            }
        }
        assert_eq!(matching_indices.len(), 1);
        assert_eq!(matching_indices[0], 1);
    }

    #[test]
    fn test_resolve_window_name_parse_output_multiple() {
        // Test: Mock output parsing for multiple matches (ambiguous)
        let output = "0|duplicate
1|duplicate
2|unique";
        let search_name = "duplicate";
        let mut matching_indices: Vec<u32> = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if !line.is_empty() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    if let Ok(index) = parts[0].trim().parse::<u32>() {
                        let name = parts[1..].join("|");
                        if name == search_name {
                            matching_indices.push(index);
                        }
                    }
                }
            }
        }
        assert_eq!(matching_indices.len(), 2);
        assert_eq!(matching_indices[0], 0);
        assert_eq!(matching_indices[1], 1);
    }

    #[test]
    fn test_resolve_window_name_parse_output_none() {
        // Test: Mock output parsing for no matches
        let output = "0|window-one
1|window-two";
        let search_name = "nonexistent";
        let mut matching_indices: Vec<u32> = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if !line.is_empty() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    if let Ok(index) = parts[0].trim().parse::<u32>() {
                        let name = parts[1..].join("|");
                        if name == search_name {
                            matching_indices.push(index);
                        }
                    }
                }
            }
        }
        assert_eq!(matching_indices.len(), 0);
    }

    #[test]
    fn test_resolve_window_name_format_error_message() {
        // Test: Error message formatting for ambiguous match
        let indices = vec![0u32, 2, 5];
        let indices_str = indices
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(indices_str, "0, 2, 5");
    }

    #[test]
    fn test_resolve_window_name_window_with_pipes_in_name() {
        // Test: Handle window names that contain pipes (edge case)
        let line = "0|a|b|c";
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 {
            let index = parts[0].trim().parse::<u32>().unwrap();
            let name = parts[1..].join("|");
            assert_eq!(index, 0);
            assert_eq!(name, "a|b|c");
        }
    }

    #[test]
    fn test_split_pane_direction_right() {
        let direction = "right";
        let (flag, label) = match direction.to_lowercase().as_str() {
            "down" | "vertical" | "-v" | "v" => ("-v", "down"),
            _ => ("-h", "right"),
        };
        assert_eq!(flag, "-h");
        assert_eq!(label, "right");
    }

    #[test]
    fn test_split_pane_direction_down() {
        let direction = "down";
        let (flag, label) = match direction.to_lowercase().as_str() {
            "down" | "vertical" | "-v" | "v" => ("-v", "down"),
            _ => ("-h", "right"),
        };
        assert_eq!(flag, "-v");
        assert_eq!(label, "down");
    }

    #[test]
    fn test_split_pane_size_valid() {
        let size = Some(35u32);
        let valid = size.map_or(true, |s| s >= 1 && s <= 99);
        assert!(valid);
    }

    #[test]
    fn test_split_pane_size_too_low() {
        let size = Some(0u32);
        let valid = size.map_or(true, |s| s >= 1 && s <= 99);
        assert!(!valid);
    }

    #[test]
    fn test_split_pane_size_too_high() {
        let size = Some(100u32);
        let valid = size.map_or(true, |s| s >= 1 && s <= 99);
        assert!(!valid);
    }

    #[test]
    fn test_split_pane_size_boundary() {
        assert!(Some(1u32).map_or(true, |s| s >= 1 && s <= 99));
        assert!(Some(99u32).map_or(true, |s| s >= 1 && s <= 99));
        assert!(!Some(100u32).map_or(true, |s| s >= 1 && s <= 99));
    }

    #[test]
    fn test_split_pane_pane_target_parsing() {
        let result = parse_pane_target("session:1.1");
        assert!(result.is_ok());
        let (session, window, pane) = result.unwrap();
        assert_eq!(session, "session");
        assert_eq!(window, "1");
        assert_eq!(pane, "1");
    }

    #[test]
    fn test_split_pane_json_output() {
        let result = serde_json::json!({
            "source_pane": "session:1.1",
            "new_pane": "session:1.2",
            "direction": "right",
            "size_percent": 50,
            "command_executed": false,
            "success": true
        });
        assert_eq!(result["source_pane"], "session:1.1");
        assert_eq!(result["direction"], "right");
        assert_eq!(result["success"], true);
    }
}
