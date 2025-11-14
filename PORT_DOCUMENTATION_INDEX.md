# Port Subcommand Documentation Index

## Overview

This directory contains comprehensive documentation for the `nabi port` subcommand - a native Rust implementation for federated port allocation management across macOS, WSL, and Raspberry Pi platforms.

## Documentation Files

### 1. **PORT_SUBCOMMAND_ANALYSIS.md** (Primary Reference)
**Purpose**: Complete architectural analysis and deep dive
**Contents**:
- Architecture & design philosophy
- Complete data structure definitions
- All 7 commands with detailed explanations
- Registry file structure and location
- Platform detection mechanisms
- Health checking implementation
- Integration with nabi-cli
- Unit tests and testing strategy
- Error handling patterns
- Performance characteristics
- Future enhancements

**Best For**: Understanding how the port subcommand works, architectural decisions, implementation details

**Size**: ~17 KB

### 2. **PORT_QUICK_REFERENCE.md** (Daily Use)
**Purpose**: Quick lookup guide for developers and operators
**Contents**:
- Command summary table
- File locations table
- Key functions by purpose
- Data structure diagrams
- Port ranges defined
- Supported platforms
- Dependencies list
- Exit codes
- Common workflows
- Testing commands
- Debugging tips

**Best For**: Quick lookups while working, copying commands, testing

**Size**: ~7 KB

### 3. **PORT_REGISTRY_ARCHITECTURE.md** (Registry Schema)
**Purpose**: Port registry schema and data model
**Contents**: (Existing comprehensive documentation)
- Registry structure
- Schema definitions
- Platform configurations
- Service specifications
- Configuration examples

**Best For**: Understanding the JSON registry format, editing port-registry.json

### 4. **PORT_REGISTRY_QUICK_REF.md** (Schema Quick Reference)
**Purpose**: Quick reference for registry editing
**Contents**: (Existing quick reference)

**Best For**: Quick lookups when editing port-registry.json

## Command Hierarchy

```
nabi port <COMMAND>
├── list [--platform PLATFORM]      # View all port allocations
├── check                           # Validate current platform
├── cross-platform                  # Check multi-platform conflicts
├── shift SERVICE OLD NEW [--dry-run]  # Migrate service to new port
├── drift [--forensic] [--since TIME]  # Analyze configuration drift
├── fix                            # Generate fix commands
└── generate-env                   # Create docker-compose .env
```

## Code Location Quick Links

| Component | File | Lines |
|-----------|------|-------|
| Main implementation | `src/commands/port.rs` | 1-994 |
| Command handler | `src/main.rs` | 3640-3687 |
| CLI definition | `src/main.rs` | 1062-1198 |
| Module export | `src/commands/mod.rs` | 5 |
| XDG path resolution | `src/paths.rs` | 1-101 |
| Unit tests | `src/commands/port.rs` | 822-985 |

## Data Source

**Port Registry File**: `/Users/tryk/.local/state/nabi/governance/port-registry.json`

**Format**: JSON with serde_json deserialization

**Size**: ~14 KB

**Structure**:
- `version`: Semantic version
- `port_ranges`: Named port allocations (3000-8499)
- `standard_allocations`: Service defaults
- `platform_configs`: Per-platform overrides (macos, wsl, rpi, linux)
- `migration_notes`: Change history
- `dynamic_services`: Runtime service registry

## Entry Points

### For Users
Start with **PORT_QUICK_REFERENCE.md**:
- See command summary
- Check common workflows
- Copy example commands

### For Developers
Start with **PORT_SUBCOMMAND_ANALYSIS.md**:
- Understand architecture
- Review data structures
- Study implementation details
- Examine test coverage

### For Operators
Use both documents:
- Quick reference for daily operations
- Analysis for troubleshooting
- Registry files for understanding configuration

## Key Features

1. **Cross-Platform Support**
   - Automatic platform detection (macOS, WSL, RPi, Linux)
   - Platform-specific configurations
   - Single unified CLI interface

2. **Safe Operations**
   - Dry-run support for migrations
   - Atomic registry writes
   - Validation before changes
   - Health checks for verification

3. **Conflict Detection**
   - Local port conflict detection
   - Cross-platform conflict analysis
   - Port drift identification
   - Automatic fix suggestions

4. **Docker Integration**
   - .env file generation
   - docker-compose compatibility
   - Container port mapping support

5. **Monitoring & Observability**
   - TCP port listening checks
   - Service health validation
   - Historical drift analysis
   - Forensic investigation tools

## Common Tasks

### Verify System Health
```bash
nabi port check              # Current platform
nabi port cross-platform     # All platforms
```

### List Current Configuration
```bash
nabi port list              # All platforms
nabi port list --platform rpi  # Specific platform
```

### Migrate Service Safely
```bash
nabi port shift service 8000 8001 --dry-run
nabi port shift service 8000 8001
```

### Fix Conflicts
```bash
nabi port drift --forensic  # Analyze
nabi port fix              # Get commands
```

### Update Docker Compose
```bash
nabi port generate-env     # Create .env.ports
```

## Dependencies

### External Crates
- `clap` 4.5 - CLI argument parsing
- `serde` / `serde_json` 1.0 - JSON serialization
- `anyhow` 1.0 - Error handling
- `colored` 2.0 - Terminal output
- `chrono` 0.4 - Timestamps
- `tokio` 1.0 - Async runtime
- `dirs` 5.0 - XDG directory support

### Internal Dependencies
- `NabiPaths` from `src/paths.rs`
- CLI framework from `main.rs`

## Testing

```bash
# Run all port tests
cargo test port::tests

# Run specific test
cargo test port::tests::test_platform_detection

# Test registry parsing
cargo test port::tests::test_port_registry_deserialization
```

## Future Roadmap

- [ ] HTTP health checks (requires `reqwest`)
- [ ] Parallel port validation (requires tokio expansion)
- [ ] Event-driven notifications
- [ ] Automatic conflict resolution
- [ ] Webhook integration for service restarts
- [ ] Commander plugin architecture

## Support & Troubleshooting

### Registry Not Found
```
Error: Port registry not found. Checked:
  - ~/.local/state/nabi/governance/port-registry.json
  - ~/.config/nabi/governance/port-registry.json

Run: nabi port rebuild
```

### Platform Not Recognized
- Ensure `/proc/version` or `/proc/cpuinfo` are readable on Linux
- Verify correct platform in registry configuration

### Port Already in Use
- Check what process is using the port
- Consider using different port range

## Architecture Philosophy

1. **Schema-Driven**: Single source of truth (port-registry.json)
2. **XDG Compliant**: Follows Linux directory standards
3. **Type Safe**: Rust's compile-time safety
4. **Observable**: Health checks and validation
5. **Safe**: Dry-run support and atomic operations
6. **Cross-Platform**: Single interface for multiple platforms
7. **Performance**: Native Rust implementation, no subprocess overhead

## Document Maintenance

- **Last Updated**: 2025-11-14
- **Author**: Codebase Analysis
- **Platform**: macOS (Darwin 25.1.0)
- **nabi-cli Version**: 0.1.0

---

## Quick Navigation

**Need to...**
- Use the port command? → `PORT_QUICK_REFERENCE.md`
- Understand the system? → `PORT_SUBCOMMAND_ANALYSIS.md`
- Edit the registry? → `PORT_REGISTRY_ARCHITECTURE.md`
- Lookup registry syntax? → `PORT_REGISTRY_QUICK_REF.md`

