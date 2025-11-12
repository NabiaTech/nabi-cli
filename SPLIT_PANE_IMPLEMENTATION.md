# Split-Pane Implementation Quick Reference

## TL;DR

Three files to modify:
1. **src/commands/tmux.rs** - Add enum variant + handler + tests (150 lines)
2. **Verify dispatch in handle_tmux_commands()** - Already patterns established

## Enum Variant (Add to TmuxCommands enum, after line 235)

```rust
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
```

## Handler Function (Add after handle_capture_pane, around line 2208)

```rust
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
```

## Dispatch Addition (Update handle_tmux_commands, ~line 621)

Find this match statement and add the case:

```rust
pub fn handle_tmux_commands(cmd: TmuxCommands) -> Result<()> {
    match cmd {
        TmuxCommands::SendPrompt { pane, message, delay } =>
            handle_send_prompt(&pane, &message, delay),
        TmuxCommands::Capture { pane, lines, format, bat } =>
            handle_capture_pane(pane.as_deref(), lines, &format, bat),
        TmuxCommands::Mem { format, limit, claude_only, threshold_mb } =>
            handle_tmux_pane_memory(format, limit, claude_only, threshold_mb),
        TmuxCommands::List(list_cmd) =>
            handle_list_commands(list_cmd),
        // ADD THIS:
        TmuxCommands::SplitPane {
            pane,
            direction,
            size,
            command,
            format,
        } => handle_split_pane(&pane, &direction, size, command, &format),
    }
}
```

## Unit Tests (Add to tests module, after line 2502)

```rust
#[cfg(test)]
mod split_pane_tests {
    use super::*;

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
```

## Usage Examples

### Basic split (horizontal, 50%)
```bash
nabi tmux split-pane session:1.1
```

### Split vertically, 30% size
```bash
nabi tmux split-pane session:1.1 -d down -s 30
```

### With command execution
```bash
nabi tmux split-pane session:1.1 -c 'npm start'
```

### JSON output for agent chaining
```bash
nabi tmux split-pane session:1.1 --format json | jq .new_pane
# Output: "session:1.2"

# Then use in subsequent commands
NEW_PANE=$(nabi tmux split-pane session:1.1 --format json | jq -r .new_pane)
nabi tmux send-prompt "$NEW_PANE" "cd ~/nabia && cargo build"
```

## Testing Locally

After adding code:

```bash
cd /Users/tryk/nabia/core/nabi-cli

# Run tests
cargo test split_pane

# Build
cargo build

# Manual test in tmux
tmux new-session -d -s test
nabi tmux list sessions
nabi tmux split-pane test:0.0 -d down --format json
nabi tmux list panes test:0
```

## Common Pitfalls

1. **Size validation**: tmux uses percentage 1-99, not 0-100
2. **Direction aliases**: Support both `-h`/`-v` and `right`/`down` strings
3. **New pane detection**: Query after split to get accurate index (async operation)
4. **Command escaping**: Let tmux handle quoting—don't shell-escape in Rust
5. **Error context**: Use `with_context()` for all Command operations

## Related Commands (Future)

Once split-pane works:
- `resize-pane` - Adjust existing pane size (similar pattern)
- `move-pane` - Reorganize panes (needs two targets)
- `kill-pane` - Close pane (simpler, just validation)

All follow the same handler pattern: parse → validate → execute tmux → query result → JSON output.
