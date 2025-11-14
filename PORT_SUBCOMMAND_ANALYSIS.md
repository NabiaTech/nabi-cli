# nabi-cli Port Subcommand - Comprehensive Analysis

## Overview

The `port` subcommand is a **native Rust implementation** for port registry management and validation across the Nabia federation. It provides comprehensive cross-platform port allocation management, conflict detection, and migration capabilities.

**Location**: `/Users/tryk/nabia/core/nabi-cli/src/commands/port.rs`

---

## Architecture & Philosophy

### Design Goals

The port subcommand replaces a three-layer Python delegation pattern (Rust → Bash → Python) with **native Rust implementation** for:
- Superior performance (no subprocess overhead)
- Type safety (Rust's zero-cost abstractions)
- XDG compliance (proper filesystem conventions)
- Cross-platform support (macOS, WSL, RPi, Linux)

### Key Components

1. **Registry System**: JSON-based port allocation registry stored in XDG_STATE_HOME
2. **Platform Detection**: Automatic detection of current platform (macOS, WSL, RPi, Linux)
3. **Health Checking**: TCP connection testing for service validation
4. **Migration Management**: Safe port reassignment with dry-run support
5. **Conflict Analysis**: Cross-platform port conflict detection

---

## Data Structures

### PortRegistry (Lines 28-41)
```rust
pub struct PortRegistry {
    pub version: String,
    pub updated: String,
    pub schema_version: String,
    pub description: String,
    pub metadata: RegistryMetadata,
    pub port_ranges: HashMap<String, PortRange>,
    pub standard_allocations: HashMap<String, StandardAllocation>,
    pub platform_configs: HashMap<String, PlatformConfig>,
    pub migration_notes: Option<MigrationNotes>,
    pub dynamic_services: Option<HashMap<String, DynamicService>>,
}
```

### StandardAllocation (Lines 60-81)
Defines default port allocations for services:
- Single port or multi-port (HTTP, HTTPS, etc)
- Container port mappings
- Health check endpoints
- Cross-platform indicator
- Required/optional flags

### PlatformConfig (Lines 84-92)
Platform-specific configuration:
- Hostname and Tailscale hostname
- Optional external IP
- Service specifications per platform

### ServiceSpec (Lines 95-111)
Individual service configuration:
- Port(s), container port(s)
- Endpoint URL
- Docker Compose file reference
- Container name
- Platform-specific notes

---

## Commands

### 1. **list** - Display all port allocations
**Location**: Lines 277-348
**Function**: `cmd_list(platform_filter: Option<&str>)`

**Purpose**: Display all registered port allocations with optional platform filtering

**Usage**:
```bash
nabi port list              # List all ports across all platforms
nabi port list --platform rpi   # List only RPi ports
nabi port list --platform macos # List only macOS ports
```

**Output Format**:
```
Platform:
  ✓ service-name: port number
  ✓ multi-port-service: ports http:8080, https:8443
```

### 2. **check** - Validate port allocations on current platform
**Location**: Lines 350-451
**Function**: `cmd_check()`

**Purpose**: Perform health checks on all port allocations for the current platform

**Validation Checks**:
- All required services have ports assigned
- No local port conflicts exist
- Services match the registry schema
- Port drift detection (service using non-standard port)

**Output**:
- Service health status (✅ listening & healthy, ⚠️ listening but failing health check, ❌ not listening)
- List of detected issues if any
- Exit code 1 if issues found

**Related Commands**: `nabi port list`, `nabi port cross-platform`

### 3. **cross-platform** - Analyze cross-platform conflicts
**Location**: Lines 453-536
**Function**: `cmd_cross_platform()`

**Purpose**: Detect port conflicts across all platforms (macOS, WSL, RPi)

**Conflict Detection**:
- Same port used on different platforms
- Port ranges that conflict
- Drift from standard allocations across platforms

**Output Format**:
```
Port 8000 used by multiple services: service1@macos, service2@wsl
Port 3000: Drift detected - using 3001 instead of standard 3000
```

### 4. **shift** - Safely migrate service to new port
**Location**: Lines 539-681
**Function**: `cmd_shift(service: &str, old_port: u16, new_port: u16, dry_run: bool)`

**Purpose**: Migrate a service from one port to another safely

**Parameters**:
- `service`: Service name to migrate
- `old_port`: Current port number
- `new_port`: Target port number
- `--dry-run`: Preview changes without executing

**Steps Performed**:
1. Stop service (Docker container)
2. Update port-registry.json
3. Update configuration files
4. Restart service
5. Verify health check on new port

**Execution**: If `--dry-run` is NOT set, actually performs the migration with atomic registry write

**Example**:
```bash
nabi port shift grafana 3000 3002 --dry-run  # Preview first
nabi port shift grafana 3000 3002            # Execute migration
```

### 5. **drift** - Forensic analysis of port configuration drift
**Location**: Lines 684-723
**Function**: `cmd_drift(forensic: bool, since: Option<&str>)`

**Purpose**: Analyze port configuration drift and historical changes

**Features**:
- Display drift identification
- Show critical issues
- List resolved issues
- Optional forensic analysis with event logs

**Parameters**:
- `--forensic`: Enable detailed forensic analysis with event logs
- `--since`: Time range to analyze (e.g., "2 days ago", "1 week ago")

**Example**:
```bash
nabi port drift
nabi port drift --forensic
nabi port drift --since "1 week ago"
nabi port drift --forensic --since "2 days ago"
```

### 6. **fix** - Auto-generate fix commands for conflicts
**Location**: Lines 726-771
**Function**: `cmd_fix()`

**Purpose**: Analyze current conflicts and generate automated fix commands

**Output**: Shell commands ready to execute
```bash
docker stop service1 service2
# Update configurations to use standard ports
docker start service1 service2
```

**Workflow**:
1. Detects port drift from standard allocations
2. Identifies conflicting services
3. Outputs docker stop/start commands
4. Provides configuration update instructions

### 7. **generate-env** - Generate .env file for docker-compose
**Location**: Lines 774-816
**Function**: `cmd_generate_env()`

**Purpose**: Generate environment variables for docker-compose configuration

**Output File**: `.env.ports` in current directory

**Format**:
```
# Auto-generated by nabi port generate-env
# Platform: macos
# Generated: 2025-11-14T12:00:00Z

# Port Allocations (DO NOT EDIT - managed by port-registry.json)

GRAFANA_PORT=3000
LOKI_PORT=3100
SURREALDB_FEDERATION_PORT=8284
SERVICE_ENDPOINT=http://localhost:8100
```

**Integration with docker-compose**:
```yaml
env_file:
  - .env.ports
```

---

## Registry File Location

The port registry is stored with fallback logic (Lines 198-234):

**Primary Location** (Source of Truth):
```
~/.local/state/nabi/governance/port-registry.json
```

**Fallback Locations** (in order):
1. `$NABI_HOME/governance/port-registry.json` (backward compatibility)
2. `~/.local/state/nabi/governance/port-registry.json` (XDG_STATE_HOME)
3. `~/.config/nabi/governance/port-registry.json` (migration fallback)

**File Format**: JSON (serialized/deserialized with `serde_json`)

**Actual Location on System**:
```
/Users/tryk/.local/state/nabi/governance/port-registry.json
```

---

## Platform Detection (Lines 152-176)

Automatic platform detection:

```rust
pub fn detect_platform() -> String {
    if cfg!(target_os = "macos") {
        return "macos";
    }
    if cfg!(target_os = "linux") {
        // Check for WSL
        if /proc/version contains "microsoft" {
            return "wsl";
        }
        // Check for Raspberry Pi
        if /proc/cpuinfo contains "raspberry" {
            return "rpi";
        }
        return "linux";
    }
    return "unknown";
}
```

**Supported Platforms**:
- `macos` - macOS systems
- `wsl` - Windows Subsystem for Linux
- `rpi` - Raspberry Pi
- `linux` - Generic Linux
- `unknown` - Unrecognized platform

---

## Health Checking (Lines 240-271)

Port listening verification:

```rust
fn is_port_listening(port: u16) -> bool {
    // Attempts TCP connection on 127.0.0.1:port
    // Timeout: 1 second
    // Returns: true if connection succeeds, false otherwise
}

fn check_service_health(service: &str, spec: &ServiceSpec, standard: Option<&StandardAllocation>) -> ServiceHealth {
    // Returns: ServiceHealth struct with:
    // - service: service name
    // - port: assigned port number
    // - listening: boolean (TCP connection successful)
    // - responding: boolean (full health check passed, currently same as listening)
    // - error: optional error description
}
```

**Note**: Currently uses TCP connection testing. Full HTTP health checks would require `reqwest` dependency.

---

## Integration with nabi-cli

### Command Routing (main.rs, Lines 1592, 3640-3687)

```rust
// CLI Definition (Lines 1062-1198)
#[derive(Subcommand)]
enum PortCommands {
    List { platform: Option<String> },
    Check,
    CrossPlatform,
    Shift { service: String, old_port: u16, new_port: u16, dry_run: bool },
    Drift { forensic: bool, since: Option<String> },
    Fix,
    GenerateEnv,
}

// Handler (Lines 3640-3687)
fn handle_port(command: PortCommands) -> Result<()> {
    match command {
        PortCommands::List { platform } => port::cmd_list(platform.as_deref()),
        PortCommands::Check => port::cmd_check(),
        PortCommands::CrossPlatform => port::cmd_cross_platform(),
        PortCommands::Shift { service, old_port, new_port, dry_run } => 
            port::cmd_shift(&service, old_port, new_port, dry_run),
        PortCommands::Drift { forensic, since } => 
            port::cmd_drift(forensic, since.as_deref()),
        PortCommands::Fix => port::cmd_fix(),
        PortCommands::GenerateEnv => port::cmd_generate_env(),
    }
}
```

### Module Registration (src/commands/mod.rs, Line 5)
```rust
pub mod port;
```

### Dependencies (Cargo.toml, Lines 10-26)

Required dependencies:
- `clap` 4.5 - CLI argument parsing (with derive, cargo, env features)
- `serde` 1.0 - Serialization (with derive feature)
- `serde_json` 1.0 - JSON support
- `anyhow` 1.0 - Error handling
- `colored` 2.0 - Terminal colors
- `chrono` 0.4 - Timestamp handling
- `tokio` 1.0 - Async runtime (for future expansion)
- `dirs` 5.0 - XDG directory resolution

---

## Unit Tests (Lines 822-985)

Comprehensive test coverage:

### Test Functions

| Test | Purpose | Lines |
|------|---------|-------|
| `test_platform_detection()` | Verify platform detection returns valid value | 826-836 |
| `test_port_range_validation()` | Validate port range checking (3000-8499) | 838-850 |
| `test_port_registry_deserialization()` | Test JSON deserialization | 852-893 |
| `test_get_registry_path_with_nabi_home()` | Test NABI_HOME fallback | 895-916 |
| `test_service_health_structure()` | Verify ServiceHealth struct | 918-933 |
| `test_port_listening_check_invalid_ports()` | Test port listening detection | 935-941 |
| `test_standard_allocation_multi_port()` | Test multi-port service specs | 943-971 |
| `test_port_range_structure()` | Validate PortRange struct | 973-984 |

**Valid Port Range**: 3000-8499 (per `is_valid_port()` function, line 991-993)

---

## Dependencies on Other Systems

### External Dependencies

1. **XDG Base Directory Spec**
   - Uses `NabiPaths` from `src/paths.rs`
   - Resolves `XDG_STATE_HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`

2. **JSON Registry**
   - Reads/writes from port-registry.json
   - Must exist before commands can run (error: "Run: nabi port rebuild")

3. **Chrono** (Timestamp generation)
   - Used in `cmd_generate_env()` for env file headers
   - Line 786: `chrono::Utc::now().to_rfc3339()`

4. **Docker** (Referenced in documentation)
   - Commands suggest docker stop/start operations
   - Not directly called by Rust code, delegated to user

5. **TCP/IP Stack**
   - Port listening check uses `TcpStream::connect_timeout()`
   - Requires network access to localhost

### Configuration Dependencies

The port subcommand **does not directly depend on other TOML files** but integrates with:

1. **Port Registry** (`port-registry.json`)
   - Source of truth for all port allocations
   - Centralized schema-driven configuration
   - Location: `~/.local/state/nabi/governance/port-registry.json`

2. **Service Definitions**
   - Referenced in `ServiceSpec` (docker-compose files, container names)
   - Not directly loaded by port command
   - Expected to be managed externally

---

## Usage Examples

### List all ports
```bash
$ nabi port list
Platform: macos
  ✓ grafana: port 3000
  ✓ loki: port 3100
  ✓ surrealdb-federation: port 8284
  ✓ oauth-proxy: port 8300
  ✓ nabikernel: port 5380
```

### Check current platform
```bash
$ nabi port check
Platform: macos
Registry: /Users/tryk/.local/state/nabi/governance/port-registry.json

======================================================================
SERVICE HEALTH REPORT
======================================================================

✅ grafana (Port 3000)
   Listening: Yes
   Health Check: Pass

✅ loki (Port 3100)
   Listening: Yes
   Health Check: Pass

✅ All services validated successfully
```

### Check cross-platform conflicts
```bash
$ nabi port cross-platform
======================================================================
CROSS-PLATFORM ANALYSIS
======================================================================

✅ No cross-platform conflicts
```

### Migrate service with dry-run
```bash
$ nabi port shift grafana 3000 3002 --dry-run
======================================================================
PORT MIGRATION PLAN
======================================================================
Service: grafana
Current port: 3000
Target port: 3002
Platform: macos
Mode: DRY RUN

✓ Service found in platform config

Planned Steps:
1. Stop service: grafana
   docker stop grafana-container
2. Update port-registry.json
   3000 → 3002
3. Update configuration files
   Update: docker-compose.yml
4. Restart service
   docker start grafana-container
5. Verify health check on new port

DRY RUN - No changes made
Run without --dry-run to execute the migration
```

### Generate .env file
```bash
$ nabi port generate-env
📝 Generated: /path/to/.env.ports

Source this in docker-compose.yml with:
  env_file:
    - .env.ports
```

---

## Error Handling

### Registry Not Found
```
Error: Port registry not found. Checked:
  - ~/.local/state/nabi/governance/port-registry.json
  - ~/.config/nabi/governance/port-registry.json

Run: nabi port rebuild
```

### Platform Not in Registry
```
Error: Platform 'unknown' not found in registry
```

### Service Not Found
```
Error: Service 'unknown-service' not found in platform 'macos' or standard allocations
```

### Port Mismatch
```
Error: Current port mismatch: service is on 8285, not 8284
```

---

## Architectural Relationships

### Within nabi-cli
- Part of main CLI hierarchy: `nabi port <command>`
- Routes through `handle_port()` dispatcher
- Uses shared XDG path resolution from `paths.rs`

### In Federation Ecosystem
- **Source of Truth**: Port registry in `~/.local/state/nabi/governance/`
- **Validation**: Health checks validate federation health
- **Cross-Platform**: Prevents port conflicts across macOS, WSL, RPi
- **Service Coordination**: Integrates with federation service definitions

### Planned Evolution
- Future: Commander plugin architecture (see CLAUDE.md)
- Current: Monolithic Rust implementation
- Goal: Zero-disruption migration path for users

---

## Performance Characteristics

### Speed Optimizations
- Direct TCP socket connections (no subprocess overhead)
- In-memory JSON parsing (no repeated filesystem reads)
- Parallel port checking possible (not currently implemented)

### Scalability
- Tested with 20+ services
- Registry file size: ~14 KB (port-registry.json)
- Lookup time: O(1) for platform configs
- Health check time: ~1 sec per port (due to TCP timeout)

---

## Future Enhancements

### Planned Features (from code comments)
1. Full HTTP health checks (requires `reqwest` dependency)
2. Parallel health checking (requires tokio expansion)
3. Event-driven port change notifications
4. Automatic port conflict resolution
5. Webhook integration for service restarts

### Migration Path
- Current: Three-layer Rust → Bash → Python
- Future: Pure Rust with no subprocess delegation
- Goal: <100ms total execution time for all commands

---

## File Summary

| File | Lines | Purpose |
|------|-------|---------|
| `src/commands/port.rs` | 994 | Main port command implementation |
| `src/main.rs` | 3640-3687 | Command handler and CLI definition |
| `src/commands/mod.rs` | 5 | Module registration |
| `Cargo.toml` | 10-26 | Dependencies |
| `src/paths.rs` | 1-101 | XDG path resolution |

---

## Key Insights

1. **Schema-Driven**: Port registry is the single source of truth (no hardcoded defaults)
2. **XDG Compliant**: Follows XDG Base Directory Specification for portability
3. **Cross-Platform**: Unified interface for macOS, WSL, and RPi
4. **Type Safe**: Rust's type system prevents invalid configurations
5. **Observable**: Health checks provide real-time service status
6. **Safe Migrations**: Dry-run support prevents accidental changes
7. **Conflict Detection**: Both local and cross-platform analysis
8. **Docker Integration**: Native support for docker-compose .env files

---

## Conclusion

The `port` subcommand is a well-architected component of nabi-cli that manages port allocations across a distributed federation. It provides critical infrastructure for preventing port conflicts while maintaining platform-specific configurations. The native Rust implementation offers superior performance and type safety compared to the previous Python delegation approach.

