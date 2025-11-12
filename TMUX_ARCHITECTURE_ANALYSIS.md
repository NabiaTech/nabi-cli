# Nabi-CLI Tmux Module Architecture Analysis

**Source**: `/Users/tryk/nabia/core/nabi-cli/src/commands/tmux.rs` (2,503 lines, 75 functions)

## Overview

The tmux module implements agent-friendly command wrapping that converts cryptic tmux output into structured JSON. Architecture emphasizes **reliability guarantees** (text + Enter atomic delivery) and **introspection** (live tmux state discovery).

## File Structure

```
src/commands/
├── tmux.rs           # Main implementation (2,503 lines)
├── mod.rs            # Module exports
├── main.rs           # Integration point
└── [other commands]  # port, kernel, init, ascii
```

**Key Integration**: `main.rs` routes `Commands::Tmux { command: TmuxCommands }` to `handle_tmux_commands()`.

---

## Architecture Pattern

### 1. **Enum-Driven Subcommand Structure**

```rust
// Lines 53-248: Main command enum
#[derive(Subcommand)]
pub enum TmuxCommands {
    SendPrompt { pane, message, delay },      // Send + execute
    Capture { pane, lines, format, bat },     // Capture pane content
    Mem { format, limit, claude_only, threshold_mb },  // Memory analysis
    List(ListCommands),                       // Introspection
}

// Lines 250-334: Nested subcommands
#[derive(Subcommand)]
pub enum ListCommands {
    Sessions { format, filter },
    Windows { session, format, no_strict },
    Panes { target, format, no_strict },
}
```

**Pattern**: Clap derives handle CLI parsing → dispatch to handler functions.

### 2. **Handler Dispatch Pattern (Lines 621-642)**

```rust
pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        TmuxCommands::SendPrompt { pane, message, delay } =>
            handle_send_prompt(&pane, &message, delay),
        TmuxCommands::Capture { pane, lines, format, bat } =>
            handle_capture_pane(...),
        TmuxCommands::Mem { ... } => handle_tmux_pane_memory(...),
        TmuxCommands::List(list_cmd) => handle_list_commands(list_cmd),
    }
}

fn handle_list_commands(cmd: ListCommands) -> Result<()> {
    match cmd {
        ListCommands::Sessions { ... } => list_tmux_sessions(...),
        ListCommands::Windows { ... } => list_tmux_windows(...),
        ListCommands::Panes { ... } => list_tmux_panes(...),
    }
}
```

**Pattern**: One handler per enum variant. Sub-handlers for nested commands.

### 3. **JSON Output Strategy**

Each handler supports `--format json` for agent consumption:

| Command | JSON Output |
|---------|------------|
| `list sessions` | `[{name, windows, attached, created, is_claude_tui, is_active}]` |
| `list windows` | `[{index, name, pane_count, is_active}]` |
| `list panes` | `[{pane, target, is_active, pane_id, send_prompt_target, tmux_target}]` |
| `capture` | `{pane, lines, timestamp, content, metadata}` |
| `mem` | `[{pane, session_type, memory_mb, memory_gb, percent_of_system, alert}]` |

**Key insight**: Multiple target format variants in pane output (send_prompt_target, tmux_target, target_shorthand) enable flexible agent chaining.

---

## Core Implementation Details

### Send Prompt (Reliable Delivery) - Lines 1939-2066

**Guarantee**: Text + Enter keypresses delivered together atomically.

**Algorithm**:
1. **Parse & validate pane format** (parse_pane_target) - supports `session:window.pane` or `session:window`
2. **Detect foreground process** - query `#{pane_current_command}` to determine recovery strategy
3. **Handle different process types**:
   - Claude TUI (versions like "2.0.34"): Send directly, no recovery
   - Shell (bash, zsh, fish): Clear continuation state with C-c
   - Blocking TUI (vim, less, python): Send C-c to exit, wait 40ms (DELAY_TUI_RECOVERY)
4. **Send text literally** - use `tmux send-keys -l message` (avoids special char interpretation)
5. **Send carriage return** - use `C-m` (not "Enter" key name) with 15ms debounce
6. **Wait for processing** - default 50ms delay (configurable via --delay)

**Timing constants** (lines 31-34):
```rust
const DELAY_TUI_RECOVERY: u64 = 40;     // Exit TUI apps
const DELAY_PROMPT_CLEAR: u64 = 20;     // Clear PS2 prompts
const DELAY_TEXT_TO_ENTER: u64 = 15;    // Text → carriage return debounce
```

### List Commands (Introspection) - Lines 1341-1740

**Pattern**: Query tmux → parse pipe-delimited output → serialize to JSON/text

**Sessions** (lines 1341-1464):
- Query: `list-sessions -F "#{session_name}|#{session_windows}|#{session_attached}|#{session_created}"`
- Detect Claude TUI: Check foreground process in first window (expensive but once per session)
- Mark as active: Compare to current session from `TMUX` env var
- Formats: `json` (default), `name` (shell completion), `full` (human), `json+context` (with invocation metadata)

**Windows** (lines 1475-1580):
- Query: `list-windows -t session -F "#{window_index}|#{window_name}|#{window_panes}|#{window_active}"`
- Session validation: Strict mode (default) errors if session != current; `--no-strict` warns
- Formats: `json`, `id`, `name`, `full` (index:name)

**Panes** (lines 1613-1740):
- Query: `list-panes -t session:window -F "#{pane_index}|#{pane_active}|#{pane_id}|#{window_id}|#{window_panes}"`
- Multiple target formats: `send_prompt_target` (for nabi tmux send-prompt), `tmux_target`, `target_shorthand`, `target_minimal`
- Formats: `json`, `id`, `number`, `full` (session:window.pane)

### Capture Pane (Content Extraction) - Lines 2072-2208

- Validate pane exists before capture
- Query current pane if none specified: `display-message -p "#{session_name}|#{window_index}|#{pane_index}"`
- Capture: `tmux capture-pane -p -t pane -S -N` (last N lines)
- Output formats: `text` (default), `json` (with metadata)
- Optional bat syntax highlighting (opt-in via --bat)

### Memory Analysis (Pane Introspection) - Lines 677-798

- Enumerate all panes: `list-panes -a` with PID
- Query `/proc` or `ps` for memory per process
- Build process tree (parent → children)
- Detect Claude TUI: Match title patterns or process names (configurable via TOML)
- Calculate RSS per pane (sum of process tree)
- Config-driven thresholds (default 1024 MB alert)

---

## Adding New Commands: Implementation Pattern

### Step 1: Define Enum Variant

Add to `TmuxCommands` enum (around line 75):

```rust
#[derive(Subcommand)]
pub enum TmuxCommands {
    // ... existing ...

    /// Split a tmux pane (horizontal or vertical)
    ///
    /// Creates a new pane by splitting the target pane.
    /// Default direction is right (-h). Use --vertical for vertical split.
    ///
    /// Common use cases:
    ///   - Creating parallel agent execution panes
    ///   - Setting up multi-agent coordination layouts
    #[command(after_help = "EXAMPLES:
  # Split pane to the right (default)
  nabi tmux split-pane session:1.1

  # Split vertically (down)
  nabi tmux split-pane session:1.1 --vertical

  # Specify percentage width
  nabi tmux split-pane session:1.1 --size 30

  # Execute command in new pane
  nabi tmux split-pane session:1.1 --command 'npm start'

DIRECTIONS:
  -h (right):   default, split horizontally
  -v (down):    split vertically")]
    SplitPane {
        /// Target pane (format: session:window.pane or session:window)
        #[arg(value_name = "PANE")]
        pane: String,

        /// Split direction: horizontal (right) or vertical (down)
        #[arg(long, default_value = "horizontal")]
        direction: String,

        /// Pane size (percentage of parent, 0-100). Default: 50
        #[arg(long)]
        size: Option<u32>,

        /// Command to execute in new pane (optional)
        #[arg(long)]
        command: Option<String>,

        /// JSON output of new pane info
        #[arg(long)]
        format: Option<String>,
    },
}
```

### Step 2: Add Handler Function

After line 642, add:

```rust
fn handle_split_pane(
    pane: &str,
    direction: &str,
    size: Option<u32>,
    command: Option<String>,
    format: Option<&str>,
) -> Result<()> {
    // Validate pane format
    let (session, window, pane_idx) = parse_pane_target(pane)
        .with_context(|| format!("Invalid pane target '{}'", pane))?;

    // Map direction string to tmux flags
    let direction_flag = match direction.to_lowercase().as_str() {
        "vertical" | "down" | "-v" => "-v",
        "horizontal" | "right" | "-h" | _ => "-h",
    };

    // Build tmux split-pane command
    let mut split_args = vec![
        "split-pane".to_string(),
        "-t".to_string(),
        pane.to_string(),
        direction_flag.to_string(),
    ];

    // Add size if specified
    if let Some(s) = size {
        if s > 0 && s < 100 {
            split_args.push("-p".to_string());
            split_args.push(s.to_string());
        }
    }

    // Add command if specified
    if let Some(cmd) = &command {
        split_args.push(cmd.clone());
    }

    // Execute split-pane
    let output = Command::new("tmux")
        .args(&split_args)
        .output()
        .context("Failed to split pane")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux split-pane failed: {}", stderr.trim());
    }

    // Query new pane info
    let list_output = Command::new("tmux")
        .args(&[
            "list-panes",
            "-t",
            &format!("{}:{}", session, window),
            "-F",
            "#{pane_index}|#{pane_id}|#{pane_active}",
        ])
        .output()
        .context("Failed to query new pane")?;

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let new_pane = stdout
        .lines()
        .last()
        .unwrap_or("") // Get last pane (usually the new one)
        .split('|')
        .next()
        .unwrap_or("");

    // Output result
    let new_pane_target = format!("{}:{}.{}", session, window, new_pane);

    match format.unwrap_or("text") {
        "json" => {
            let result = serde_json::json!({
                "source_pane": pane,
                "new_pane": new_pane_target,
                "direction": direction,
                "command_executed": command.is_some(),
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("✓ Split pane '{}': {}", pane, new_pane_target);
        }
    }

    Ok(())
}
```

### Step 3: Add Dispatch in Handler

Update `handle_tmux_commands()` (line 621):

```rust
pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        // ... existing ...
        TmuxCommands::SplitPane {
            pane,
            direction,
            size,
            command,
            format,
        } => handle_split_pane(&pane, &direction, size, command, format.as_deref()),
    }
}
```

### Step 4: Add Tests

Around line 2210:

```rust
#[test]
fn test_split_pane_direction_mapping() {
    assert_eq!(match "vertical".to_lowercase().as_str() {
        "vertical" | "down" | "-v" => "-v",
        _ => "-h",
    }, "-v");

    assert_eq!(match "right".to_lowercase().as_str() {
        "vertical" | "down" | "-v" => "-v",
        _ => "-h",
    }, "-h");
}

#[test]
fn test_split_pane_size_validation() {
    let size = Some(50u32);
    assert!(size.unwrap_or(0) > 0 && size.unwrap_or(0) < 100);
}
```

---

## Other Potentially Useful Commands

| Command | Purpose | Implementation Complexity |
|---------|---------|---------------------------|
| `resize-pane` | Adjust pane dimensions | Medium (size parsing, bounds validation) |
| `select-pane` | Focus a pane | Low (just target + flags) |
| `kill-pane` | Close a pane | Low (target validation only) |
| `move-pane` | Reorganize panes | Medium (source + dest targets, validation) |
| `select-window` | Focus a window | Low (session + window parsing) |
| `new-window` | Create window | Medium (name, command, layout) |
| `kill-window` | Close window | Low (session + window parsing) |
| `list-keys` | Show keybindings | Low (just query tmux) |

**Recommendation**: Start with `resize-pane` (heavily used, simple) before `move-pane`.

---

## Key Design Insights

### Error Handling Pattern

```rust
// Use `with_context()` for error augmentation
Command::new("tmux")
    .args(&args)
    .output()
    .with_context(|| format!("Failed to X: {}", description))?;

// Combine result matching with bail! for clarity
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("command failed: {}", stderr.trim());
}
```

### Pane Target Parsing

The `parse_pane_target()` function (lines 1749-1798) is the **canonical parser** for all pane references. Reuse it—don't regex.

```rust
parse_pane_target(pane)?  // Returns (session, window, pane)
```

### Context Detection Pattern

Introspect current tmux state before operating:

```rust
let context = get_current_tmux_context()?;
// Check: context.in_tmux, context.session_name, context.pane_index, etc.
```

### Process Detection

For intelligent behavior (Claude TUI vs blocking apps vs shells):

```rust
let fg_process = Command::new("tmux")
    .args(&["display-message", "-p", "-t", pane, "#{pane_current_command}"])
    .output()?;
let process_name = String::from_utf8_lossy(&fg_process.stdout).trim().to_string();

if is_claude_tui(&process_name) { /* ... */ }
if needs_tui_recovery(&process_name) { /* ... */ }
```

---

## JSON Output Contracts (Agent-Friendly)

### Sessions Format
```json
{
  "name": "session-name",
  "windows": 3,
  "attached": true,
  "created": "2025-11-11T15:30:45+00:00",
  "is_claude_tui": true,
  "is_active": true
}
```

### Panes Format
```json
{
  "pane": 0,
  "target": "session:window.pane",
  "is_active": false,
  "pane_id": "%0",
  "window_id": "@0",
  "send_prompt_target": "session:1.0",
  "tmux_target": "session:1.0",
  "target_shorthand": ":1.0",
  "target_minimal": ":1"
}
```

---

## Testing Strategy

### Unit Tests (Existing: 49 tests)

Focus on parsing and validation:
- `parse_pane_target()` - format validation
- `is_claude_tui()` - process detection
- `needs_tui_recovery()` - TUI classification
- `resolve_window_name()` - exact-match window lookup

### Integration Tests (Recommended)

Test against live tmux session (CI environment dependent):
- Send command and verify pane state change
- Capture and validate output format
- Split/kill operations and list refreshes

---

## Summary

**To add commands**: Define enum → handler function → dispatch → tests. Reuse `parse_pane_target()`, `get_current_tmux_context()`, and JSON serialization patterns. The architecture prioritizes **reliability** (atomic send-prompt), **introspection** (live state queries), and **agent compatibility** (clean JSON output).
