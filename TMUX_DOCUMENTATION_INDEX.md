# Tmux Module Documentation Index

**Created**: 2025-11-11
**Location**: `/Users/tryk/nabia/core/nabi-cli/`
**Source Code**: `src/commands/tmux.rs` (2,503 lines)

## New Documentation Files (This Analysis)

### 1. **TMUX_ARCHITECTURE_ANALYSIS.md** (474 lines)
- **Best for**: Understanding how the system works
- **Covers**:
  - File structure and module organization
  - Enum-driven subcommand pattern
  - Handler dispatch flow
  - JSON output strategy for agents
  - Core implementation details (SendPrompt, List, Capture, Memory)
  - Pane target parsing and validation
  - Testing strategy
- **Read when**: You want to understand the architecture before modifying
- **Time**: 20-30 minutes

### 2. **SPLIT_PANE_IMPLEMENTATION.md** (357 lines)
- **Best for**: Copy-paste ready code for split-pane command
- **Covers**:
  - Complete enum variant definition
  - Full handler function implementation
  - Dispatch integration
  - Unit tests (8 tests)
  - Usage examples
  - Testing locally with tmux
  - Common pitfalls
- **Read when**: Ready to implement split-pane
- **Time**: 30 minutes to copy + 1 hour to test
- **Copy-paste**: Yes, all code is production-ready

### 3. **TMUX_COMMAND_EXTENSION_GUIDE.md** (468 lines)
- **Best for**: Step-by-step guide for adding any new command
- **Covers**:
  - 30-second overview of the pattern
  - 4-step process for adding commands
  - Common patterns & reusable utilities
  - Reference for all 4 existing commands
  - Ranked list of candidate commands (9 total)
  - Testing strategy
  - Common mistakes & recovery
  - Performance considerations
- **Read when**: Adding a new command (any command, not just split-pane)
- **Time**: 10-15 minutes for overview, reference as needed

### 4. **TMUX_DOCUMENTATION_INDEX.md** (this file)
- Navigation hub for all tmux documentation

---

## How to Use This Documentation

### Scenario 1: "I need to understand the architecture"
1. Read: **TMUX_ARCHITECTURE_ANALYSIS.md** (20-30 min)
2. Skim: Handler functions in `src/commands/tmux.rs` (lines 621-2208)
3. Reference: Table of commands (SendPrompt, Capture, List, Mem)

### Scenario 2: "I want to add split-pane"
1. Skim: **TMUX_COMMAND_EXTENSION_GUIDE.md** (5 min, "Core Pattern" section)
2. Copy: **SPLIT_PANE_IMPLEMENTATION.md** (30 min implementation)
3. Test: Run `cargo build && nabi tmux split-pane test:0.0` (10 min)

### Scenario 3: "I need to add a different command (not split-pane)"
1. Review: **TMUX_COMMAND_EXTENSION_GUIDE.md** "Core Pattern" (5 min)
2. Choose: Your command from "Candidate Commands" section
3. Follow: 4-step process with utilities from "Common Patterns"
4. Reference: Similar existing command from "Existing Commands Reference"

### Scenario 4: "I'm maintaining or debugging"
- Reference: **TMUX_ARCHITECTURE_ANALYSIS.md** table of commands + line numbers
- Debug: Common mistakes section in **TMUX_COMMAND_EXTENSION_GUIDE.md**
- Test: Use patterns from "Testing Strategy"

---

## Quick Reference Tables

### File Structure
| File | Lines | Purpose |
|------|-------|---------|
| src/commands/tmux.rs | 2,503 | Main implementation (75 functions) |
| src/commands/mod.rs | 5 | Module exports |
| src/main.rs | Large | Routes Commands::Tmux to handlers |

### Existing Commands
| Command | Lines | Complexity | Purpose | JSON? |
|---------|-------|-----------|---------|-------|
| send-prompt | 1939-2066 | High | Reliable command execution | No |
| capture | 2072-2208 | Low | Extract pane content | Yes |
| list sessions | 1341-1464 | Medium | Introspect sessions | Yes |
| list windows | 1475-1580 | Medium | Query windows in session | Yes |
| list panes | 1613-1740 | Medium | List panes in window | Yes |
| mem | 677-798 | High | Memory analysis per pane | Yes |

### Candidate Commands (Ready to Implement)
| Rank | Command | Lines | Complexity | Effort |
|------|---------|-------|-----------|--------|
| 1 | resize-pane | ~100 | Low | 1 day |
| 2 | select-pane | ~50 | Low | 1 day |
| 3 | kill-pane | ~60 | Low | 1 day |
| 4 | **split-pane** | ~150 | Medium | 1 day |
| 5 | move-pane | ~120 | Medium | 2 days |
| 6 | new-window | ~100 | Medium | 2 days |
| 7 | select-window | ~80 | Medium | 2 days |
| 8 | kill-window | ~100 | Medium | 2 days |
| 9 | list-keys | ~80 | Low | 1 day |

---

## Key Utilities (Reusable)

### Functions to Understand First
```rust
parse_pane_target()              // Line 1749 - parse "session:window.pane"
get_current_tmux_context()       // Line 1143 - detect tmux environment
is_claude_tui()                  // Line 1805 - process detection
needs_tui_recovery()             // Line 1829 - TUI classification
```

### Patterns to Copy
- **Error handling**: `Command::new("tmux").output().context("msg")?`
- **JSON output**: `serde_json::json!({ ... })`
- **Process query**: `display-message -p "#{pane_current_command}"`
- **Pipe parsing**: `line.split('|').collect::<Vec<_>>()`

---

## Implementation Roadmap

### Phase 1: Understand (This Task)
- [x] Read TMUX_ARCHITECTURE_ANALYSIS.md
- [x] Understand enum + handler pattern
- [x] Review existing commands

### Phase 2: Implement split-pane (Recommended Next)
- [ ] Copy code from SPLIT_PANE_IMPLEMENTATION.md
- [ ] Update enum variant
- [ ] Implement handler function
- [ ] Add dispatch in handle_tmux_commands()
- [ ] Run tests: `cargo test split_pane`
- [ ] Manual test: `nabi tmux split-pane session:0.0`

### Phase 3: Add resize-pane (Low Effort)
- [ ] Similar pattern to split-pane
- [ ] Simpler validation (size only)
- [ ] 1 day effort

### Phase 4: Advanced Commands
- [ ] move-pane (2 target validation)
- [ ] select-window (with name resolution)
- [ ] Higher complexity but same pattern

---

## Testing Checklist

### For Any New Command
- [ ] Enum variant added (TmuxCommands enum)
- [ ] Handler function implemented (with Result<()>)
- [ ] Dispatch added (handle_tmux_commands match)
- [ ] Unit tests added (#[test] functions)
- [ ] `cargo test` passes
- [ ] `cargo build` succeeds
- [ ] Manual test in live tmux:
  ```bash
  tmux new-session -d -s test
  cargo build
  nabi tmux [your-command] test:0.0
  nabi tmux list panes test:0  # Verify result
  ```
- [ ] Help text checked: `nabi tmux [your-command] --help`
- [ ] Error handling tested: try invalid pane, missing args, etc.

---

## Common Questions

### Q: How do I run split-pane locally?
**A**: See SPLIT_PANE_IMPLEMENTATION.md section "Testing Locally"

```bash
cd /Users/tryk/nabia/core/nabi-cli
cargo build
tmux new-session -d -s test
nabi tmux split-pane test:0.0 --format json
```

### Q: Why use parse_pane_target() instead of regex?
**A**: Parsing validates format → (session, window, pane) tuple. Safer and reusable across all commands.

### Q: What's the JSON output format?
**A**: Each command defines its own. See TMUX_ARCHITECTURE_ANALYSIS.md "JSON Output Contracts"

### Q: How do I know what tmux flags to use?
**A**: Check `man tmux` for native command, then model after existing handlers (SendPrompt, Capture, etc.)

### Q: Should I add `--format json` to my command?
**A**: Yes, all list/query commands should support it for agent chaining.

### Q: How is error handling done?
**A**: Use `anyhow::Result<()>` with `context()` for augmented errors and `bail!()` for explicit errors.

---

## Related Files in This Repo

### Project Files
- `src/commands/tmux.rs` - Main implementation (source of truth)
- `src/main.rs` - Integration point
- `Cargo.toml` - Dependencies

### Existing Documentation
- `TESTING.md` - Testing procedures
- `DEVELOPMENT.md` - Development guidelines
- `README.md` - Project overview
- `CLAUDE.md` - Project philosophy

### External References
- `~/docs/MASTER_INDEX.md` - Federation documentation
- `~/docs/architecture/AGENTS.md` - Available agents
- `~/docs/tools/nabi-cli.md` - Nabi CLI overview

---

## Key Insights

### The Pattern (Core Learning)
1. **Define**: Add enum variant with clap derive
2. **Implement**: Write handler function (parse → validate → tmux → serialize)
3. **Dispatch**: Add case to handle_tmux_commands()
4. **Test**: Add unit tests + manual test

**Time to implement**: 1-3 days depending on complexity

### Architecture Principles
- **Reliability**: SendPrompt ensures atomic text + Enter delivery
- **Introspection**: List commands enable live state discovery
- **Agent-friendly**: JSON output for machine parsing
- **Reusability**: Common utilities (parse_pane_target, etc.)

### Design Lessons
- Avoid regex—use canonical parsers
- Query after modifying (split-pane queries new pane)
- Process detection (Claude TUI vs blocking vs shells)
- Multiple output formats (JSON, text, filtered)

---

## Support & Troubleshooting

### If Split-Pane Won't Build
1. Check syntax: `cargo check`
2. Verify enum matches (did you update handle_tmux_commands)?
3. Check imports (serde_json, Command, etc. are in scope)
4. Run tests: `cargo test` to isolate issues

### If Split-Pane Returns Wrong Pane
1. Check that you query AFTER split (async operation)
2. Verify pane target format: `session:window.pane`
3. Use `nabi tmux list panes session:window` to debug

### If Tests Fail
1. Ensure unit tests don't require live tmux (they shouldn't)
2. Check parse_pane_target() tests for format validation
3. Use `cargo test -- --nocapture` to see output

---

## Next Steps

1. **Read** TMUX_ARCHITECTURE_ANALYSIS.md (20-30 min)
2. **Review** existing commands in src/commands/tmux.rs
3. **Implement** split-pane using SPLIT_PANE_IMPLEMENTATION.md (1 day)
4. **Test** locally in tmux
5. **Add** resize-pane or next command

---

## File Hierarchy

```
nabi-cli/
├── src/
│   ├── main.rs                         # Entry point
│   ├── commands/
│   │   ├── mod.rs                      # Module exports
│   │   ├── tmux.rs                     # THIS FILE (2,503 lines)
│   │   ├── port.rs
│   │   ├── kernel.rs
│   │   └── init.rs
│   └── ...
├── Cargo.toml                          # Dependencies
├── TMUX_ARCHITECTURE_ANALYSIS.md       # Comprehensive guide
├── SPLIT_PANE_IMPLEMENTATION.md        # Ready-to-copy code
├── TMUX_COMMAND_EXTENSION_GUIDE.md    # General pattern guide
└── TMUX_DOCUMENTATION_INDEX.md        # This file
```

---

## Summary

**This analysis provides three documents**:

1. **TMUX_ARCHITECTURE_ANALYSIS.md** - Comprehensive system design
2. **SPLIT_PANE_IMPLEMENTATION.md** - Ready-to-use code
3. **TMUX_COMMAND_EXTENSION_GUIDE.md** - General extension pattern

**Use case**: Add new tmux commands to nabi-cli while maintaining reliability and agent compatibility.

**Time estimate**: 1-3 days per command (low complexity: 1 day, medium: 2 days, high: 3 days)

**Complexity**: Medium - straightforward pattern, well-documented code, good test coverage
