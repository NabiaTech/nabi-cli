# Tmux Command Extension Guide

## Quick Navigation

- **Architecture Deep Dive**: See `/TMUX_ARCHITECTURE_ANALYSIS.md` (comprehensive)
- **Split-Pane Implementation**: See `/SPLIT_PANE_IMPLEMENTATION.md` (ready-to-copy code)
- **Source Code**: `src/commands/tmux.rs` (2,503 lines, well-documented)

---

## 30-Second Overview

The nabi-cli tmux module is a **three-part wrapper system**:

1. **CLI Definition** (clap enums) - Parse user input
2. **Handlers** (functions) - Call `tmux` binary + parse output
3. **JSON Serialization** - Output clean data for agents

**Key insight**: All handlers follow the same pattern:
```
parse input → validate → execute tmux command → parse output → serialize JSON
```

---

## Core Pattern: Adding a Command

### 1. Define Enum Variant

Add to `TmuxCommands` enum in `src/commands/tmux.rs` (line ~75):

```rust
#[derive(Subcommand)]
pub enum TmuxCommands {
    SendPrompt { /* ... */ },
    Capture { /* ... */ },
    Mem { /* ... */ },
    List(ListCommands),

    // YOUR NEW COMMAND HERE:
    /// Brief description
    #[command(after_help = "EXAMPLES:\n  ...")]
    YourCommand {
        #[arg(value_name = "REQUIRED_ARG")]
        required_arg: String,

        #[arg(long, default_value = "default")]
        optional_flag: String,
    },
}
```

### 2. Write Handler Function

After `handle_capture_pane()` (line ~2208):

```rust
fn handle_your_command(required_arg: &str, optional_flag: &str) -> Result<()> {
    // Step 1: Validate inputs
    if required_arg.is_empty() {
        anyhow::bail!("required_arg cannot be empty");
    }

    // Step 2: Execute tmux command
    let output = Command::new("tmux")
        .args(&["subcommand", required_arg, optional_flag])
        .output()
        .context("Failed to execute tmux subcommand")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux subcommand failed: {}", stderr.trim());
    }

    // Step 3: Parse and return result
    let stdout = String::from_utf8_lossy(&output.stdout);
    // ... parse output, serialize to JSON if needed ...

    println!("✓ Command succeeded");
    Ok(())
}
```

### 3. Add Dispatch

Update `handle_tmux_commands()` (line ~621):

```rust
pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        // ... existing ...
        TmuxCommands::YourCommand {
            required_arg,
            optional_flag,
        } => handle_your_command(&required_arg, &optional_flag),
    }
}
```

### 4. Add Tests

In `#[cfg(test)] mod tests` (line ~2211):

```rust
#[test]
fn test_your_command_validates_input() {
    let result = handle_your_command("", "");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}
```

---

## Common Patterns & Utilities

### Pane Target Parsing

**Don't regex**. Use the canonical parser:

```rust
let (session, window, pane) = parse_pane_target(pane_spec)?;
// Returns: (String, String, String)
// Accepts: "session:window.pane" or "session:window" (pane defaults to 1)
```

Tests at line 2215-2279 show all valid formats.

### Current Context Detection

Check if running inside tmux:

```rust
let context = get_current_tmux_context()?;
if context.in_tmux {
    println!("Current session: {:?}", context.session_name);
    println!("Pane index base: {}", context.pane_index_base);
}
```

### Process Detection

Determine what's running in a pane:

```rust
let fg_process = Command::new("tmux")
    .args(&["display-message", "-p", "-t", pane, "#{pane_current_command}"])
    .output()?;
let process_name = String::from_utf8_lossy(&fg_process.stdout).trim();

if is_claude_tui(process_name) { /* Direct input */ }
if needs_tui_recovery(process_name) { /* Send Ctrl+C first */ }
```

See lines 1805-1832 for implementations.

### JSON Output

All list/query commands support `--format json`:

```rust
match format {
    "json" => {
        let result = serde_json::json!({
            "field1": "value1",
            "field2": value2,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    "text" | _ => println!("Human-readable output"),
}
```

### Error Handling

Use `anyhow::Result<()>` with context:

```rust
let output = Command::new("tmux")
    .args(&args)
    .output()
    .context("Failed to query tmux")?;  // Adds context to error

if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("Command failed: {}", stderr.trim());  // Explicit error
}
```

---

## Existing Commands Reference

### SendPrompt (Lines 1939-2066)

**Purpose**: Reliably execute a command in a pane

**Guarantees**: Text + Enter delivered atomically

**Complexity**: High (process detection, recovery logic, timing)

**Key learning**: Multi-step tmux operations with recovery

### Capture (Lines 2072-2208)

**Purpose**: Extract pane content (last N lines)

**Complexity**: Low (straightforward tmux call + formatting)

**Key learning**: JSON with metadata, bat integration

### List Commands (Lines 1341-1740)

**Purpose**: Query tmux state (sessions, windows, panes)

**Complexity**: Medium (parsing, multiple formats, validation)

**Key learning**: Pipe-delimited parsing, multiple output formats

### Mem (Lines 677-798)

**Purpose**: Analyze memory per pane

**Complexity**: High (multi-process introspection, system queries)

**Key learning**: Process tree walking, threshold logic

---

## Candidate Commands (Ranked by Difficulty)

### Tier 1: Simple (1-2 days)

**resize-pane**: Adjust pane dimensions
- Tmux command: `resize-pane -t pane -x WIDTH -y HEIGHT` or `-D N` (delta)
- Validation: width/height > 0, or delta non-zero
- Output: Success message (no introspection needed)

**select-pane**: Focus a pane (change active pane)
- Tmux command: `select-pane -t pane`
- Validation: pane exists (use parse_pane_target)
- Output: Current pane info

**kill-pane**: Close a pane
- Tmux command: `kill-pane -t pane`
- Validation: pane exists, not the only pane in window
- Output: Confirmation

### Tier 2: Medium (2-3 days)

**split-pane**: Create new pane by splitting
- Tmux command: `split-pane -t pane [-h|-v] [-p SIZE] [COMMAND]`
- Validation: size 1-99%, direction normalized
- Output: New pane info (needs query after split)
- See: `/SPLIT_PANE_IMPLEMENTATION.md` for complete code

**move-pane**: Relocate pane to different window
- Tmux command: `move-pane -s source_pane -t target_window`
- Validation: both source and target exist
- Output: New pane target
- Complexity: two pane targets, introspection after move

**new-window**: Create window in session
- Tmux command: `new-window -t session [-n name] [COMMAND]`
- Validation: session exists, name unique (optional)
- Output: New window info

### Tier 3: Complex (3+ days)

**select-window**: Focus a window (switch active window)
- Same as select-pane but for windows
- Add window name resolution: `:={name}` syntax (see lines 1852-1937)

**kill-window**: Close a window (with safety checks)
- Prevent last window deletion
- Complex because affects multiple panes

**list-commands**: Show all tmux keybindings
- Tmux command: `list-keys`
- Format and filter output
- Lower priority (less agent-useful)

---

## Testing Strategy

### Unit Tests (No tmux required)

```rust
#[test]
fn test_parse_and_validate() {
    let result = parse_pane_target("session:1.1");
    assert!(result.is_ok());

    let result = parse_pane_target("invalid");
    assert!(result.is_err());
}
```

**Coverage**: Input validation, error cases, formatting

### Integration Tests (Requires tmux)

```bash
# Manually test in tmux
tmux new-session -d -s test
cargo build
nabi tmux split-pane test:0.0 --format json
nabi tmux list panes test:0
```

**Coverage**: Actual tmux integration, command execution

### CI Strategy

Existing tests (~49 unit tests, line 2211+) don't require live tmux. Add integration tests only if CI has tmux available.

---

## Key Files Reference

| File | Lines | Purpose |
|------|-------|---------|
| src/commands/tmux.rs | 2,503 | Main implementation |
| src/commands/mod.rs | 5 | Module export |
| src/main.rs | Large | Routes Commands::Tmux to handler |
| TMUX_ARCHITECTURE_ANALYSIS.md | 400+ | This file's source documentation |
| SPLIT_PANE_IMPLEMENTATION.md | 300+ | Copy-paste ready code |

---

## Deployment Notes

### Building

```bash
cd /Users/tryk/nabia/core/nabi-cli
cargo build --release
# Binary: target/release/nabi
```

### Symlinks

The nabi CLI is typically symlinked into `~/.nabi/bin/` or `~/bin/` for access.

### Help Text

Always include `after_help` in command definitions with EXAMPLES. This shows up in:
```bash
nabi tmux split-pane --help
```

---

## Common Mistakes & Recovery

### 1. Forgetting Pane Validation

**Wrong**:
```rust
fn handle_foo(pane: &str) -> Result<()> {
    // ... directly use pane ...
}
```

**Right**:
```rust
fn handle_foo(pane: &str) -> Result<()> {
    let (_session, _window, _pane) = parse_pane_target(pane)?;
    // Now safe to use pane
}
```

### 2. Not Handling tmux Failures

**Wrong**:
```rust
let output = Command::new("tmux").args(&args).output()?;
let stdout = String::from_utf8_lossy(&output.stdout);
```

**Right**:
```rust
let output = Command::new("tmux").args(&args).output()?;
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("tmux failed: {}", stderr.trim());
}
let stdout = String::from_utf8_lossy(&output.stdout);
```

### 3. Regex Instead of Parsing

**Wrong**:
```rust
if pane.contains(":") && pane.contains(".") { /* assume valid */ }
```

**Right**:
```rust
let (session, window, pane) = parse_pane_target(pane)?;
// Guaranteed valid format
```

### 4. Hardcoding Pipe-Delimited Parsing

**Wrong**:
```rust
let parts = line.split('|').collect::<Vec<_>>();
let value = parts[2]; // What if format changes?
```

**Right**:
```rust
let parts: Vec<&str> = line.split('|').collect();
if parts.len() >= 3 {
    let value = parts[2];
    // Safe with bounds checking
}
```

---

## Performance Considerations

### Expensive Operations

- **Session Claude detection** (~1 pane query per session) - cached in list-sessions
- **Memory introspection** (~1 ps query + all panes) - OK for infrequent use
- **Window name resolution** (one list-windows query) - used only when explicit `:={name}` syntax

### Optimization Opportunities

- Cache pane lists (query once, reuse)
- Batch multiple `list-panes` calls into single session query
- Use `-F` format strings to minimize output parsing

---

## Related Documentation

- **NORTH STAR**: Cognitive architecture principles (~/docs/NORTH_STAR.md)
- **Nabi CLI**: Overall design (~/docs/tools/nabi-cli.md)
- **Path Resolution**: XDG compliance patterns (~/docs/infrastructure/PATH_RESOLUTION_CANONICAL.md)
- **Agent Roster**: Available federation agents (~/docs/architecture/AGENTS.md)

---

## Next Steps

1. **Review** `/SPLIT_PANE_IMPLEMENTATION.md` for ready-to-copy code
2. **Understand** parsing pattern via `parse_pane_target()` tests (lines 2215-2279)
3. **Implement** split-pane following the three-part pattern
4. **Test** with live tmux: `cargo build && nabi tmux split-pane test:0.0`
5. **Add** resize-pane (similar complexity, highly useful)
6. **Plan** next command based on agent needs

---

## Support & Questions

Refer back to this guide and the architecture analysis. The codebase is well-commented with examples throughout tmux.rs.

Key functions to study:
- `parse_pane_target()` (1749-1798)
- `handle_send_prompt()` (1939-2066)
- `list_tmux_panes()` (1613-1740)
- `handle_capture_pane()` (2072-2208)
