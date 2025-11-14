# Port Subcommand - Quick Reference Guide

## Command Summary

| Command | Function | Purpose |
|---------|----------|---------|
| `nabi port list [--platform PLATFORM]` | cmd_list() | Display all port allocations |
| `nabi port check` | cmd_check() | Validate ports on current platform |
| `nabi port cross-platform` | cmd_cross_platform() | Check cross-platform conflicts |
| `nabi port shift SERVICE OLD_PORT NEW_PORT [--dry-run]` | cmd_shift() | Migrate service to new port |
| `nabi port drift [--forensic] [--since TIME]` | cmd_drift() | Analyze port configuration drift |
| `nabi port fix` | cmd_fix() | Auto-generate fix commands |
| `nabi port generate-env` | cmd_generate_env() | Generate .env for docker-compose |

## File Locations

| File | Purpose | Absolute Path |
|------|---------|---------------|
| Main implementation | Port command code | `/Users/tryk/nabia/core/nabi-cli/src/commands/port.rs` |
| CLI dispatcher | Command routing | `/Users/tryk/nabia/core/nabi-cli/src/main.rs` (lines 3640-3687) |
| CLI definition | Argument parsing | `/Users/tryk/nabia/core/nabi-cli/src/main.rs` (lines 1062-1198) |
| Port registry | Data source | `/Users/tryk/.local/state/nabi/governance/port-registry.json` |
| Module export | Registry | `/Users/tryk/nabia/core/nabi-cli/src/commands/mod.rs` (line 5) |
| Path resolver | XDG support | `/Users/tryk/nabia/core/nabi-cli/src/paths.rs` |
| Tests | Unit tests | `/Users/tryk/nabia/core/nabi-cli/src/commands/port.rs` (lines 822-985) |

## Key Functions by Purpose

### Data Loading
- **load_registry()** (line 182): Load and parse JSON registry
- **get_registry_path()** (line 198): Resolve registry location with fallbacks
- **detect_platform()** (line 152): Auto-detect current platform

### Health Checking
- **is_port_listening()** (line 240): Test if port has active listener
- **check_service_health()** (line 248): Full health check for service

### Core Commands (994 lines)
1. **cmd_list** - Lines 277-348 (72 lines)
2. **cmd_check** - Lines 350-451 (102 lines)
3. **cmd_cross_platform** - Lines 453-536 (84 lines)
4. **cmd_shift** - Lines 539-681 (143 lines)
5. **cmd_drift** - Lines 684-723 (40 lines)
6. **cmd_fix** - Lines 726-771 (46 lines)
7. **cmd_generate_env** - Lines 774-816 (43 lines)

## Data Structures

### Main Registry Structure
```
PortRegistry {
  version: String,
  updated: String,
  schema_version: String,
  metadata: RegistryMetadata,
  port_ranges: HashMap<String, PortRange>,              // Named ranges (3000-8499)
  standard_allocations: HashMap<String, StandardAllocation>,  // Service defaults
  platform_configs: HashMap<String, PlatformConfig>,    // Per-platform overrides
  migration_notes: Option<MigrationNotes>,              // Change history
  dynamic_services: Option<HashMap<String, DynamicService>>,  // Runtime services
}
```

### Service-Level Structure
```
PlatformConfig {
  hostname: String,
  tailscale_hostname: String,
  external_ip: Option<String>,
  role: Option<String>,
  services: HashMap<String, ServiceSpec>,
}

ServiceSpec {
  enabled: bool,
  port: Option<u16>,                    // Single port
  ports: Option<HashMap<String, u16>>,  // Multi-port (http, https, etc)
  container_port: Option<u16>,
  endpoint: Option<String>,
  compose_file: Option<String>,
  container_name: Option<String>,
  note: Option<String>,
}
```

## Port Ranges Defined

| Range | Start | End | Purpose |
|-------|-------|-----|---------|
| federation_core | 8000 | 8099 | MCP, coordination, core services |
| applications | 8100 | 8199 | Application services (Vigil, etc) |
| infrastructure | 3000 | 3999 | Loki, Grafana, monitoring |
| storage | 8200 | 8299 | Databases (SurrealDB, Redis, etc) |
| development | 8400 | 8499 | Local dev servers, testing |
| sync_communication | 8300 | 8399 | OAuth, Syncthing, communication |

## Supported Platforms

| Platform | Detection Method | When Recognized |
|----------|-----------------|-----------------|
| macos | `cfg!(target_os = "macos")` | Always on macOS |
| wsl | `/proc/version` contains "microsoft" | Linux with WSL kernel |
| rpi | `/proc/cpuinfo` contains "raspberry" | Raspberry Pi hardware |
| linux | Default Linux detection | Generic Linux |
| unknown | Fallback | Unrecognized platform |

## Dependencies

### Crate Dependencies
```toml
clap = "4.5"        # CLI parsing
serde = "1.0"       # Serialization
serde_json = "1.0"  # JSON support
anyhow = "1.0"      # Error handling
colored = "2.0"     # Terminal colors
chrono = "0.4"      # Timestamps
tokio = "1.0"       # Async runtime
dirs = "5.0"        # XDG directories
```

### Internal Dependencies
- `NabiPaths` from `src/paths.rs` - XDG path resolution
- `port` module exported in `src/commands/mod.rs`
- Handler wired in `main()` via `handle_port()`

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Validation failure (e.g., required services not listening) |
| 1 | Registry not found |
| 1 | Service not found |
| 1 | Port mismatch error |

## Important Notes

1. **Registry is Source of Truth**: No hardcoded defaults; everything comes from JSON
2. **XDG Compliant**: Uses standard Linux directory conventions
3. **Cross-Platform**: Single interface for macOS, WSL, RPi
4. **Safe Migrations**: Always use `--dry-run` before actual migration
5. **TCP Port Checking**: 1-second timeout per port
6. **Platform-Specific**: Registry maintains per-platform overrides
7. **Docker Integration**: Generates .env files for docker-compose

## Common Workflows

### 1. Verify All Services Are Running
```bash
nabi port check
```

### 2. Find Cross-Platform Port Conflicts
```bash
nabi port cross-platform
```

### 3. Safely Migrate a Service
```bash
nabi port shift myservice 8000 8001 --dry-run
nabi port shift myservice 8000 8001
nabi port check  # Verify success
```

### 4. Update docker-compose
```bash
nabi port generate-env > .env.ports
# Add to docker-compose.yml:
# env_file:
#   - .env.ports
```

### 5. Diagnose Port Drift
```bash
nabi port drift --forensic
nabi port fix  # Get recommended commands
```

## Testing

Run all port-related tests:
```bash
cd /Users/tryk/nabia/core/nabi-cli
cargo test port::tests
```

Test specific functionality:
```bash
cargo test port::tests::test_platform_detection
cargo test port::tests::test_port_range_validation
cargo test port::tests::test_port_registry_deserialization
```

## Future Enhancements

- [ ] Full HTTP health checks (needs `reqwest` dependency)
- [ ] Parallel port checking (needs tokio expansion)
- [ ] Event-driven notifications
- [ ] Automatic conflict resolution
- [ ] Webhook integration
- [ ] Commander plugin architecture (planned)

## Debugging

### Enable Verbose Output
Most commands already use colored terminal output.

### Check Registry Validity
```bash
cat ~/.local/state/nabi/governance/port-registry.json | jq '.'
```

### Test Registry Path Resolution
```bash
# Create test program using NabiPaths::state_dir()
```

### Manual Port Check
```bash
nc -zv localhost 8000  # Test if port 8000 is listening
```

---

**Created**: 2025-11-14
**Platform**: macOS (Darwin 25.1.0)
**nabi-cli Version**: 0.1.0

