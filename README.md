# nabi-cli - Federation Command Gateway

**Status**: Foundation Complete ✅
**Ready for**: Commander Implementation
**Date**: 2025-10-05 01:06 AM

## Quick Start

```bash
# nabi is installed and operational
nabi --help
nabi self doctor
nabi self config

# Example commands (routing implemented, commanders pending):
nabi claude session list
nabi data jsonl validate file.jsonl
nabi federation agent list
```

## Architecture Summary

```
~/.config/nabi/
├── cli/                    # ✅ Main router (Rust, 637KB binary)
├── commanders/
│   ├── claude/            # ⏳ Pending: Extract from claude-manager
│   ├── data/              # ⏳ Pending: Consolidate JSONL tools
│   └── federation/        # ⏳ Pending: Wire existing governance
├── governance/            # ✅ Existing federation infrastructure
└── hooks/                 # ✅ Existing Claude Code hooks

~/.local/bin/nabi          # ✅ Installed system-wide
```

## Next Implementation Steps

### Campaign Alpha: Data Forge (JSONL Consolidation)
**Priority**: High - Immediate value for data operations

1. **Import jsonl-validator** (Rust core from V: drive)
   ```bash
   cp -r /mnt/v/nabia-ai-stack/tools/jsonl-validator \
         ~/.config/nabi/commanders/data/
   ```

2. **Extract JSONL viewer** (Python from riff-cli)
   ```bash
   cp ~/nabia/projects/riff-cli/python/jsonl_tool.py \
      ~/.config/nabi/commanders/data/viewer.py
   ```

3. **Extract recovery logic** (TypeScript from claude-manager)
   - Analyze `~/nabia/projects/claude-manager/cm-recover.ts`
   - Extract generic JSONL parsing logic
   - Move Claude-specific logic to claude-commander

4. **Create unified interface**
   ```rust
   // ~/.config/nabi/commanders/data/main.rs
   // Dispatch to: validate, repair, view subcommands
   ```

### Campaign Beta: Claude Commander
**Priority**: Medium - Leverage existing claude-manager

1. **Analyze claude-manager boundaries**
   - Session operations → Keep in claude-commander
   - JSONL operations → Move to data-forge
   - Generic utilities → Evaluate for extraction

2. **Create claude-commander wrapper**
   ```typescript
   // ~/.config/nabi/commanders/claude/index.ts
   // Import existing claude-manager, expose as commander
   ```

### Campaign Gamma: Plugin Protocol
**Priority**: Medium - Enable extensibility

1. **Define JSON protocol**
   ```json
   {
     "command": "claude session list",
     "args": {"limit": 10},
     "flags": {"verbose": true}
   }
   ```

2. **Implement subprocess communication**
   - stdin/stdout for data
   - stderr for errors
   - Exit codes for status

## Testing the Foundation

### Current State
```bash
# Router works, shows what would be executed:
$ nabi claude session list --limit 5
📋 Listing Claude sessions...
→ Would route to commander: claude ["session", "list", "--limit", "5"]
ℹ️  Commander implementation pending

# Health check validates structure:
$ nabi self doctor
🏥 Running health check...
  ✓ claude present
  ✓ data present
  ✓ federation present
✓ All commanders healthy!
```

### When Commanders Are Implemented
```bash
# Will execute actual commands via subprocess:
$ nabi data jsonl validate memory.jsonl
✓ Validating memory.jsonl...
[Actual validator output]

$ nabi claude session recover 57eea7e7
🔄 Recovering session 57eea7e7...
[Actual recovery output]
```

## Development Workflow

### Building
```bash
cd ~/.config/nabi/cli
cargo build --release
cp target/release/nabi ~/.local/bin/
```

### Adding New Commands
1. Edit `src/main.rs` to add command enum
2. Add handler function
3. Update routing logic
4. Rebuild and test

### Adding New Commander
1. Create directory: `~/.config/nabi/commanders/new-name/`
2. Implement executable that handles JSON protocol
3. nabi will auto-detect and route

## JSONL Tools Inventory

Ready to consolidate into data-forge:

| Tool | Location | Purpose | Language |
|------|----------|---------|----------|
| **jsonl-validator** | `/mnt/v/nabia-ai-stack/tools/jsonl-validator/` | Repair broken JSONL | Rust ✅ |
| **jsonl_tool.py** | `~/nabia/projects/riff-cli/python/jsonl_tool.py` | Interactive viewer | Python |
| **cm-recover.ts** | `~/nabia/projects/claude-manager/cm-recover.ts` | Session recovery (extract JSONL logic) | TypeScript |

## Success Metrics

Foundation Phase ✅:
- [x] XDG-compliant structure
- [x] Rust router skeleton
- [x] Command routing system
- [x] Health check system
- [x] System-wide installation

Commander Phase (Next):
- [ ] data-forge operational
- [ ] claude-commander operational
- [ ] federation-core operational
- [ ] Full backward compatibility

## The Noble Promise

> "Router, not monolith. Coordination, not control."
> — Igris, Chief Strategist

Transform scattered tools into unified federation command structure:
- ✅ Honor existing investments (governance, hooks preserved)
- ✅ Clear evolution path (parallel campaigns defined)
- 🔄 Backward compatibility (via routing and symlinks)
- ✅ Future extensibility (plugin architecture ready)
- ✅ Platform conventions (XDG compliance)

---

**When you're ready to start**: Begin with Campaign Alpha (data-forge) for immediate value.

**Quick win**: Import jsonl-validator to data-forge → Wire to nabi → Test `nabi data jsonl validate`

## XDG-Compliant Build System

This project follows XDG Base Directory Specification for cross-platform compatibility.

### Directory Structure

```
~/nabia/core/nabi-cli/     # Source code (this repository)
~/.cache/nabi/nabi-cli/    # Build artifacts (XDG_CACHE_HOME)
~/.local/bin/nabi          # Installed binary (XDG_BIN_HOME)
```

### Building

Use the Makefile for XDG-compliant builds:

```bash
# Generate .cargo/config.toml from template
make config

# Build release binary
make build

# Install to ~/.local/bin
make install

# Quick rebuild and install
make quick

# Clean build artifacts
make clean
```

### Manual Build

If you prefer cargo directly:

```bash
# Generate config first
make config

# Then use cargo
cargo build --release
cp ~/.cache/nabi/nabi-cli/target/release/nabi ~/.local/bin/nabi
```

### Cross-Platform Notes

- The `.cargo/config.toml` file is generated from `.cargo/config.toml.template`
- XDG paths are resolved at build time via Makefile
- macOS: Uses `~/Library/Application Support` conventions wrapped as XDG
- Linux/WSL: Uses standard XDG paths (`~/.cache`, `~/.local/share`)

