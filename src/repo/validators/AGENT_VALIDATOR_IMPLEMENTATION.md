# Agent Command Profile Validator Implementation

## Overview

The `agent.rs` module provides schema-driven validation of agent command profiles against `~/.config/nabi/validation/agent-rules.toml`.

## Architecture

### Core Components

1. **AgentRulesConfig** - Deserializes TOML configuration
   - Maps rule definitions from TOML
   - Tracks ignore patterns and clean files
   - Provides config reloading

2. **Rule** - Individual validation rule
   - `rule_id`: Unique identifier (e.g., AGENT-PATH-001)
   - `name`: Human-readable rule name
   - `severity`: HIGH, MEDIUM, or LOW-MEDIUM
   - `patterns`: Array of patterns to detect
   - `regex`: Boolean flag for regex vs literal matching
   - `target_files`: Glob patterns for which files to scan
   - `migration_guidance`: Help text for fixing violations

3. **AgentValidator** - Implements Validator trait
   - Loads config from ~/.config/nabi/validation/agent-rules.toml
   - Scans ~/.claude/commands/ and ~/.xdg/claude-config/commands/
   - Applies all rules to matching files
   - Collects and sorts violations

### Detection Workflow

```
Load agent-rules.toml
    ↓
Scan command directories
    ↓
For each file:
    - Check if matches target_files patterns
    - Skip if in clean_files list
    - For each rule:
        - For each pattern:
            - Apply regex or literal match
            - Skip comment/code block lines
            - Collect violations
    ↓
Sort violations by file and line
    ↓
Return violations
```

## Integration Points

### In check.rs

```rust
let validators: Vec<Box<dyn Validator>> = vec![
    Box::new(XdgValidator),
    Box::new(PathValidator::new()),
    Box::new(SymlinkValidator::new()),
    Box::new(AgentValidator::new()),  // ← New validator
];
```

### In validators/mod.rs

```rust
pub mod agent;  // ← Exported module
```

## Rule Detection Examples

### Example 1: Hardcoded Paths (AGENT-PATH-001)

**Rule Configuration:**
```toml
[rules.hardcoded_user_paths]
rule_id = "AGENT-PATH-001"
severity = "HIGH"
patterns = ["/Users/[^/]+/", "/home/[^/]+/"]
regex = true
target_files = ["**/*.md", "**/*.sh"]
```

**Detection:**
- File: `repo-inbox-intake.md`
- Line 10: `/Users/tryk/leGen/inbox`
- Violation: AGENT-PATH-001 detected

### Example 2: Deprecated Memchain Path (AGENT-MCP-001)

**Rule Configuration:**
```toml
[rules.deprecated_memchain_path]
rule_id = "AGENT-MCP-001"
severity = "HIGH"
patterns = ["~/.memchain", "memchain/"]
regex = false
target_files = ["**/*.md", "**/*.yaml"]
```

**Detection:**
- File: `onboard.md`
- Line 101: `cat ~/.memchain/federation/agent.json`
- Violation: AGENT-MCP-001 detected

### Example 3: Deprecated MCP Server (AGENT-MCP-002)

**Rule Configuration:**
```toml
[rules.deprecated_mcp_memchain]
rule_id = "AGENT-MCP-002"
severity = "MEDIUM"
patterns = ["mcp__memchain__"]
regex = false
target_files = ["**/*.md"]
```

**Detection:**
- File: `analyze.md`
- Line 4: `tools: mcp__memchain__store`
- Violation: AGENT-MCP-002 detected

## Configuration Format

### Minimal agent-rules.toml Structure

```toml
[rules.rule_name]
rule_id = "AGENT-XXX-NNN"
name = "Human readable name"
severity = "HIGH" | "MEDIUM" | "LOW-MEDIUM"
description = "What this rule detects"
patterns = ["pattern1", "pattern2"]
regex = true | false
target_files = ["**/*.md", "**/*.sh"]
migration_guidance = "How to fix this violation"

[ignore_patterns]
clean_files = ["atomic.md", "bootstrap.md"]
```

## Severity Mapping

| TOML Value  | Severity Enum | Exit Code |
|-------------|---------------|-----------|
| HIGH        | Critical      | 3         |
| MEDIUM      | Warning       | 1 (strict) |
| LOW-MEDIUM  | Warning       | 1 (strict) |

## Features

### Pattern Matching

- **Literal patterns**: Simple substring search (`~/.memchain`)
- **Regex patterns**: Full regex support (`/Users/[^/]+/`)
- **Glob file matching**: `*.md`, `*.yaml`, `**/*.sh`

### False Positive Filtering

Automatically skips lines:
- Starting with `#` (shell comments)
- Starting with `//` (code comments)
- Starting with `<!--` (HTML comments)
- Starting with ` ``` ` (code blocks)
- Starting with `---` (dividers)

### Directory Scanning

Scans in order:
1. `~/.claude/commands/` (primary location)
2. `~/.xdg/claude-config/commands/` (fallback)

Only processes: `.md`, `.yaml`, `.yml`, `.sh` files

## Output Format

### Console Output (Text)

```
agent_commands validator... ✓ (15 issues)

─── WARNING ─────────────────────────────────────────────
  ▸ repo-inbox-intake.md:10
    [AGENT-PATH-001] Hardcoded user home paths in agent commands: /Users/tryk/ detected
    💡 Replace with ~/ or $HOME for portability
```

### JSON Output

```json
{
  "violations": [
    {
      "file": "repo-inbox-intake.md",
      "line": 10,
      "rule_id": "AGENT-PATH-001",
      "severity": "Critical",
      "message": "Hardcoded user home paths...",
      "suggestion": "Replace with ~/ or..."
    }
  ]
}
```

## Testing

Unit tests included for:
- Glob pattern matching (exact, wildcard, prefix)
- Line skipping (comments, code blocks)
- Severity mapping (HIGH → Critical, etc.)

Run tests:
```bash
cargo test repo::validators::agent
```

## Performance Characteristics

- **Config load**: ~1-2ms (single TOML parse)
- **Directory scan**: ~50-200ms (depends on file count)
- **Pattern matching**: ~0.1-1ms per file (depends on pattern complexity)
- **Total**: ~100-500ms for typical agent command directory

## Error Handling

- Missing agent-rules.toml: Returns empty violation list (no-op)
- Invalid regex patterns: Logged to stderr, skipped
- File read errors: Logged, validation continues
- Missing command directories: Silently skipped

## Integration with nabi-cli

Use via `nabi repo check`:

```bash
# Scan current repo (includes agent validators)
nabi repo check .

# JSON output
nabi repo check . --format json

# Strict mode (warnings cause non-zero exit)
nabi repo check . --strict

# From any directory (uses HOME environment variable)
nabi repo check ~/nabia
```

## Future Enhancements

1. CLI filtering: `--severity HIGH` to filter violations
2. File pattern filtering: `--file "*.md"` to scan only certain files
3. Rule selection: `--rules AGENT-PATH-001,AGENT-MCP-001`
4. Auto-fix suggestions: Generate sed commands for migration
5. Integration with Linear: Create issues for violations
6. Dry-run mode: Show what would be fixed
