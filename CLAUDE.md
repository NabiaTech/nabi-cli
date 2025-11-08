# CLAUDE.md - nabi-cli Federation Gateway

⚠️ **ARCHITECTURE STATUS**:
- **This Document**: Commander plugin architecture (**FUTURE ROADMAP** - not yet implemented)
- **Current Implementation**: Three-layer polyglot routing (Rust → Bash → Python)
- **Source Location**: Moving from `~/.config/nabi/cli/` → `~/nabia/core/nabi-cli/` (Convergence Phase 2)
- **Current State Documentation**: See `~/Sync/docs/architecture/UNIFIED_CONVERGENCE_PLAN.md`

**Note**: This document preserves the architectural vision for future evolution. For the current operational architecture, see the global `~/.claude/CLAUDE.md` "Execution Topology" section.

---

## 🎯 Architectural Vision (Future Roadmap)

**nabi-cli** is the supreme orchestrator - a unified federation command gateway that routes to specialized commanders. Following Igris's noble vision: **router, not monolith**.

## Core Philosophy

```yaml
design_principles:
  single_entry_point: "nabi command as federation gateway"
  specialized_commanders: "Each domain has clear authority"
  plugin_architecture: "New commanders without core changes"
  zero_disruption: "Backward compatibility with existing tools"
  xdg_compliance: "Proper platform conventions"
```

## Architecture

### Command Hierarchy

```
nabi                        # Root gateway (Rust binary)
├── claude                  # Routes to claude-commander
│   ├── session            # Session management
│   ├── project            # Project operations
│   └── recover            # Recovery & repair
├── data                    # Routes to data-forge
│   ├── jsonl              # JSONL operations
│   ├── json               # JSON operations
│   └── transform          # Format conversion
├── federation              # Routes to federation-core
│   ├── agent              # Agent coordination
│   ├── memory             # Memory operations
│   └── monitor            # Federation monitoring
└── self                    # Self-management
    ├── update             # Update all commanders
    ├── doctor             # Health check
    └── config             # Configuration
```

### Directory Structure

**⚠️ POST-CONVERGENCE STRUCTURE** (after Convergence Phase 2):

```
~/nabia/core/nabi-cli/     # Main CLI router source (XDG-compliant)
├── src/                   # Rust source code
├── Cargo.toml             # Dependencies
└── target/ → ~/.cache/nabi/nabi-cli/target/  # Build artifacts symlink

~/.local/bin/nabi          # Installed binary (XDG-compliant)

~/.config/nabi/            # Configuration only (no source code)
├── governance/            # Federation governance
├── hooks/                 # Claude Code hooks
├── lib/                   # Python utilities (doctor, syncthing)
└── auras/                 # Aura TOML schemas

~/.local/share/nabi/       # Persistent data (XDG_DATA_HOME)
├── bin/                   # Federated tool binaries
└── venvs/                 # Python virtual environments

~/.local/state/nabi/       # Runtime state (XDG_STATE_HOME)
├── manifests/             # Manifest tracking
└── federation/            # Federation state

~/.cache/nabi/             # Disposable cache (XDG_CACHE_HOME)
└── nabi-cli/target/       # Build artifacts

~/.nabi/                   # Unified access layer (symlinks)
├── source → ~/nabia/core/ # Development reference
└── bin → ~/.local/share/nabi/bin/  # Tool access
```

**FUTURE VISION** (Commander Plugin Architecture):

```
~/.config/nabi/commanders/  # Commander plugins (not yet implemented)
│   ├── claude/            # Claude domain authority
│   ├── data/              # Data operations authority
│   └── federation/        # Federation coordination
```

## Technology Stack

```yaml
nabi_cli:
  core: Rust                # Performance, single binary
  cli_framework: clap       # Argument parsing
  plugins: "Subprocess communication (any language)"

commanders:
  claude: "TypeScript/Bun + Rust TUI"
  data: "Rust core + language bindings"
  federation: "Python + Rust extensions"
```

## Development Roadmap

### Phase 1: Foundation (Current)
- [x] XDG directory structure
- [ ] Rust CLI skeleton with clap
- [ ] Plugin protocol design
- [ ] Commander routing system

### Phase 2: Data Commander
- [ ] Import jsonl-validator (Rust)
- [ ] Extract JSONL logic from claude-manager
- [ ] Extract viewer from riff-cli
- [ ] Unified data-forge binary

### Phase 3: Claude Commander
- [ ] Analyze claude-manager boundaries
- [ ] Extract Claude-specific operations
- [ ] Integration with nabi router
- [ ] Backward compatibility layer

### Phase 4: Federation Integration
- [ ] Wire existing federation/governance
- [ ] Agent coordination commands
- [ ] Memory operations interface
- [ ] Monitoring integration

### Phase 5: Migration
- [ ] Symlink legacy commands
- [ ] Documentation update
- [ ] 6-month deprecation notices
- [ ] User migration guide

## Commander Protocol

### Interface Contract

Each commander must provide:

```json
{
  "name": "commander-name",
  "version": "1.0.0",
  "commands": [
    {
      "name": "command-name",
      "description": "What it does",
      "args": ["required-args"],
      "flags": ["--optional-flags"]
    }
  ],
  "health_check": "commander-name health",
  "update": "commander-name update"
}
```

### Communication Protocol

```rust
// nabi-cli spawns commanders as subprocesses
// Input: JSON via stdin
// Output: JSON via stdout
// Errors: JSON via stderr

{
  "command": "claude session list",
  "args": {"limit": 10},
  "flags": {"verbose": true}
}
```

## Integration Points

### With Existing Tools

```bash
# Backward compatibility via symlinks
cm -> nabi claude           # claude-manager
riff -> nabi data           # riff-cli utilities
```

### With Federation Governance

```bash
# Leverage existing infrastructure
~/.config/nabi/governance/  # Federation protocols
~/.config/nabi/hooks/       # Claude Code hooks
~/.config/nabi/federation/  # Agent registry
```

## Usage Examples

```bash
# Session management (replaces cm)
nabi claude session list
nabi claude session recover 57eea7e7
nabi claude project migrate ~/new-path

# Data operations (consolidates JSONL tools)
nabi data jsonl validate file.jsonl
nabi data jsonl repair file.jsonl --fix
nabi data jsonl view file.jsonl --query "search term"

# Federation coordination
nabi federation agent list
nabi federation agent spawn worker-001
nabi federation memory query "concept"

# Self-management
nabi self doctor              # Health check all commanders
nabi self update              # Update all components
nabi self config show         # Show configuration
```

## Success Metrics

1. **Zero Workflow Disruption**: All existing commands work
2. **Discoverability**: 90% of features via `nabi help`
3. **Performance**: 50%+ faster via Rust core
4. **Adoption**: 50% migration in first month
5. **Maintenance**: 70% less code duplication

## Contributing

### Adding a New Commander

1. Create directory: `~/.config/nabi/commanders/new-commander/`
2. Implement protocol interface (see above)
3. Register in `nabi-cli/src/commanders/mod.rs`
4. Add routing in `nabi-cli/src/router.rs`
5. Update help text and documentation

### Building

**⚠️ POST-CONVERGENCE PATHS** (after Convergence Phase 2):

```bash
# New source location (XDG-compliant)
cd ~/nabia/core/nabi-cli
cargo build --release

# Build artifacts (XDG_CACHE_HOME)
# Symlinked: ~/.cache/nabi/nabi-cli/target/

# Binary installed to (XDG-compliant)
cp target/release/nabi ~/.local/bin/nabi
```

**PRE-CONVERGENCE PATHS** (current, temporary):

```bash
# Old location (will move in Phase 2)
cd ~/.config/nabi/cli
cargo build --release

# Binary at: target/release/nabi
# Install to: ~/.local/bin/nabi
```

## The Noble Promise

Transform scattered tools into unified federation command structure:
- Honor existing investments
- Provide clear evolution path
- Maintain backward compatibility
- Enable future extensibility
- Follow platform conventions

*"Router, not monolith. Coordination, not control."*
*— Igris, Chief Strategist*

---

**Status**: Foundation Phase (Active Development)
**Next**: Rust CLI skeleton with plugin routing
