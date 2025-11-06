# Nabi Port Registry Implementation Analysis

## Executive Summary

The nabi port command implements a **schema-driven port management system** following the Aura pattern (Config → Transform → Derived State). It uses a two-layer architecture:

1. **Configuration Layer**: TOML files in `~/.config/nabi/ports/` (source of truth)
2. **Derived State Layer**: JSON registry in `~/.local/state/nabi/ports/registry.json` (runtime)

The implementation splits cleanly between:
- **Rust** (Commands, validation, conflict detection, health checking)
- **Python** (TOML transformation, port conflict analysis)

---

## 1. CURRENT IMPLEMENTATION (port.rs)

### Data Structures

The Rust implementation defines the complete port registry model:

```rust
pub struct PortRegistry {
    pub version: String,
    pub updated: String,
    pub schema_version: String,
    pub description: String,
    pub metadata: RegistryMetadata,
    pub port_ranges: HashMap<String, PortRange>,           // 3000-3999, 8000-8099, etc
    pub standard_allocations: HashMap<String, StandardAllocation>, // Per-service standard config
    pub platform_configs: HashMap<String, PlatformConfig>,  // Platform-specific overrides
    pub migration_notes: Option<MigrationNotes>,            // Drift tracking
    pub dynamic_services: Option<HashMap<String, DynamicService>>, // Runtime registry
}
```

**Key Field: `platform_configs`** - The critical data structure for multi-platform support:

```rust
pub struct PlatformConfig {
    pub hostname: String,
    pub tailscale_hostname: String,
    pub external_ip: Option<String>,
    pub role: Option<String>,
    pub services: HashMap<String, ServiceSpec>,  // Per-platform service config
}

pub struct ServiceSpec {
    pub enabled: bool,
    pub port: Option<u16>,          // Single port
    pub ports: Option<HashMap<String, u16>>,  // Multiple ports (e.g., http:8080, ws:8081)
    pub container_port: Option<u16>,
    pub endpoint: Option<String>,
    pub compose_file: Option<String>,
    pub container_name: Option<String>,
    pub note: Option<String>,
}
```

### Subcommands

| Command | Purpose | Status |
|---------|---------|--------|
| `list` | List port allocations (with optional --platform filter) | ✅ Full |
| `check` | Validate port allocations against running processes | ✅ Full |
| `cross-platform` | Check for cross-platform port conflicts | ✅ Full |
| `shift` | Safely migrate service to new port | ⚠️ Dry-run only |
| `drift` | Forensic analysis of port drift | ✅ Full |
| `fix` | Auto-generate fix commands for conflicts | ✅ Full |
| `generate-env` | Generate docker-compose .env file | ✅ Full |

### Key Functions

#### Registry Loading (lines 177-209)

```rust
pub fn load_registry() -> Result<PortRegistry> {
    let registry_path = get_registry_path()?;
    let content = fs::read_to_string(&registry_path)?;
    let registry: PortRegistry = serde_json::from_str(&content)?;
    Ok(registry)
}

fn get_registry_path() -> Result<PathBuf> {
    // Checks NABI_HOME env var first
    // Falls back to ~/.config/nabi/governance/port-registry.json
    // NOTE: NOT XDG-compliant yet - should be ~/.local/state/nabi/ports/
}
```

**ISSUE**: Registry path points to `~/.config/nabi/governance/` (config dir) but should read from `~/.local/state/nabi/ports/registry.json` (state dir).

#### Platform Detection (lines 147-171)

```rust
pub fn detect_platform() -> String {
    if cfg!(target_os = "macos") { "macos" }
    else if cfg!(target_os = "linux") {
        if /proc/version contains "microsoft" { "wsl" }
        else if /proc/cpuinfo contains "raspberry" { "rpi" }
        else { "linux" }
    }
    else { "unknown" }
}
```

Detects: **macos**, **wsl**, **rpi**, **linux**, **unknown**

#### Health Checking (lines 223-242)

```rust
fn check_service_health(service: &str, spec: &ServiceSpec, 
    standard: Option<&StandardAllocation>) -> ServiceHealth {
    let port = spec.port.unwrap_or(0);
    let listening = if port > 0 { is_port_listening(port) } else { false };
    // Health check: Simple TCP connect (no HTTP health endpoint)
    ServiceHealth { service, port, listening, responding: listening, error: None }
}
```

**Limitation**: Only checks TCP connectivity, doesn't verify HTTP health endpoints.

#### Multi-Port Service Support (lines 270-276)

```rust
if let Some(ports) = &service_spec.ports {
    let ports_str: Vec<String> = ports.iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
    println!("  {} {}: ports {}", status, service_name, ports_str.join(", "));
}
```

**Status**: Partially implemented in `list` command. Not fully integrated into validation/conflict detection.

#### Drift Detection (lines 365-374)

```rust
if let Some(standard_port) = standard_alloc.port {
    if let Some(actual_port) = service_spec.port {
        if standard_port != actual_port {
            issues.push(format!(
                "⚠️  {}: Using port {}, standard is {} (drift detected)",
                service_name, actual_port, standard_port
            ));
        }
    }
}
```

Detects deviations from standard allocations.

---

## 2. TRANSFORM PIPELINE (transform_ports.py)

The Python transformation system converts TOML → JSON registry.

### Data Flow

```
TOML Config Files                    Validation
(~/.config/nabi/ports/*.toml)  →  (Range, conflicts)  →  JSON Registry
  - infrastructure.toml                                    (derived state)
  - federation_core.toml                    +  Manifest
  - applications.toml                       (tracking)
  - storage.toml
  - communication.toml
  - _ranges.toml (reference)
```

### PortTransformer Class

#### Initialization (lines 38-45)

```python
def __init__(self, platform: Optional[str] = None):
    self.home = Path.home()
    self.config_dir = self.home / ".config" / "nabi" / "ports"
    self.state_dir = self.home / ".local" / "state" / "nabi" / "ports"  # XDG-compliant
    self.platform = platform or self._detect_platform()
    self.state_dir.mkdir(parents=True, exist_ok=True)
```

**XDG Compliance**: ✅ Correctly uses `~/.local/state/nabi/ports/`

#### Validation Logic (lines 76-126)

Three-layer validation:

1. **Port Range Validation** (lines 76-85)
   - Checks port ∈ [3000, 8499]
   - Validates port is integer type

2. **Service Config Validation** (lines 87-125)
   - Required fields: `port`, `purpose`
   - Optional: `container_port`, `platform` overrides
   - Protocol validation: tcp, udp, http, https, websocket, sse
   - Supports combined protocols: "http+sse", "ws+http+sse"

3. **Port Conflict Detection** (lines 127-157)
   ```python
   def check_port_conflicts(self, all_configs) -> Tuple[List[str], Dict[str, List[str]]]:
       port_map = defaultdict(list)  # port -> [service names]
       
       for service_name, config in all_configs.items():
           # Get effective port (with platform override resolution)
           port = config.get("port")
           platforms = config.get("platforms", {})
           if self.platform in platforms:
               platform_config = platforms[self.platform]
               if "port" in platform_config:
                   port = platform_config["port"]
           
           # Detect duplicates
           if len(port_map[port]) > 1:
               errors.append(f"Port {port} allocated to multiple services")
   ```

#### TOML Service Transformation (lines 159-202)

Transforms platform-specific TOML structure:

```toml
[services.loki]
port = 3100
protocol = "http"
purpose = "Federation event logging"
required = true
cross_platform = false
preferred_host = "rpi"

[services.loki.platforms.wsl]
enabled = true
port = 3101  # Platform override
endpoint = "http://localhost:3101"
```

Into JSON registry entry:

```json
{
  "name": "loki",
  "port": 3100,
  "protocol": "http",
  "purpose": "Federation event logging",
  "required": true,
  "platforms": {
    "wsl": {
      "enabled": true,
      "port": 3101,
      "endpoint": "http://localhost:3101"
    }
  }
}
```

#### Registry Output (lines 204-230)

Generates `~/.local/state/nabi/ports/registry.json` with metadata:

```json
{
  "generated_at": "ISO8601",
  "transformer_version": "1.0.0",
  "platform": "macos",
  "config_location": "/Users/tryk/.config/nabi/ports",
  "services": { ... },
  "metadata": {
    "total_services": 14,
    "enabled_services": 14,
    "ranges": {
      "infrastructure": "3000-3999",
      "federation_core": "8000-8099",
      "applications": "8100-8199",
      "storage": "8200-8299",
      "communication": "8300-8399",
      "development": "8400-8499"
    }
  }
}
```

#### Manifest Generation (lines 232-258)

Tracks which TOML files were loaded during transformation:

```json
{
  "generated_at": "ISO8601",
  "transformer_version": "1.0.0",
  "platform": "macos",
  "configs_loaded": [
    "applications.toml",
    "storage.toml",
    "infrastructure.toml",
    "communication.toml",
    "federation_core.toml"
  ],
  "services": {
    "vigil_cmp_cognitive": { "port": 8101, "enabled": true },
    ...
  }
}
```

---

## 3. VALIDATION PIPELINE (validate_ports.py)

Completes the Aura pattern with reality checks.

### PortValidator Class

#### Registry Discovery (lines 53-61)

```python
def _load_registry(self) -> Dict:
    # Reads from ~/.local/state/nabi/ports/registry.json
    # Fails if registry not found (requires prior transformation)
```

#### Running Port Detection

Two mechanisms:

1. **lsof-based** (lines 63-106) - macOS/Linux system processes
   - Parses `lsof -i -P -n` output
   - Extracts port, process name, connection state

2. **Docker-based** (lines 108-154) - Containerized services
   - Parses `docker ps` output
   - Maps external:internal port mappings
   - Identifies container image/ID

#### Drift Detection (lines 184-215)

```python
def check_drift(self) -> Tuple[List[str], List[str], List[str]]:
    running_all = {**running_lsof, **running_docker}  # All running ports
    declared = self.get_declared_ports()  # From registry
    
    # Category 1: Declared but not running
    for port, service_name in declared.items():
        if port not in running_all:
            if service.get("required", False):
                errors.append(...)  # Critical drift
            else:
                warnings.append(...)  # Warning
    
    # Category 2: Running but not declared
    for port, info in running_all.items():
        if port not in declared:
            info.append(...)  # Undeclared service detected
```

Returns: **(errors, warnings, info)** tuple for categorized drift.

---

## 4. SCHEMA-DRIVEN ARCHITECTURE

### Aura Pattern Implementation

```
┌─────────────────────────────────────────────────────┐
│ SCHEMA LAYER (TOML Configs)                        │
│ ~/.config/nabi/ports/                              │
│  ├─ _ranges.toml (port ranges reference)          │
│  ├─ infrastructure.toml (3000-3999)                │
│  ├─ federation_core.toml (8000-8099)               │
│  └─ ... (5 more files)                             │
└──────────────┬──────────────────────────────────────┘
               │ transform_ports.py
               ↓ (Validate, Transform)
┌──────────────────────────────────────────────────────┐
│ DERIVED STATE (JSON Registry)                       │
│ ~/.local/state/nabi/ports/                         │
│  ├─ registry.json (14 services, all platforms)     │
│  └─ manifest.json (transformation audit)           │
└──────────────┬──────────────────────────────────────┘
               │ validate_ports.py & port.rs
               ↓ (Reality Check)
┌──────────────────────────────────────────────────────┐
│ VALIDATION RESULTS                                  │
│  ├─ Errors (required services not running)         │
│  ├─ Warnings (optional services not running)       │
│  ├─ Info (undeclared services running)             │
│  └─ Port drift (actual vs declared)                │
└──────────────────────────────────────────────────────┘
```

### Entry Points

| Command | Tool | Layer | Purpose |
|---------|------|-------|---------|
| `nabi port list` | Rust | Derived State | Display registry |
| `nabi port check` | Rust | Validation | Verify running services |
| `nabi port cross-platform` | Rust | Validation | Check multi-platform conflicts |
| (manual) `python transform_ports.py` | Python | Transform | TOML → JSON |
| (manual) `python validate_ports.py` | Python | Validation | Reality check |

---

## 5. XDG COMPLIANCE ANALYSIS

### Current State

| Layer | Path | XDG Compliant | Notes |
|-------|------|---------------|-------|
| Config | `~/.config/nabi/ports/*.toml` | ✅ Yes | Correct XDG_CONFIG_HOME |
| State | `~/.local/state/nabi/ports/*.json` | ✅ Yes | Correct XDG_STATE_HOME |
| Rust Load | `~/.config/nabi/governance/port-registry.json` | ❌ NO | Should read from state dir |

### Bug Found

**`port.rs` line 189-208**: Registry path resolution is **incorrect**:

```rust
fn get_registry_path() -> Result<PathBuf> {
    // ❌ BUG: Reads from ~/.config/nabi/governance/
    // but registry is generated in ~/.local/state/nabi/ports/
    
    let path = home.join(".config").join("nabi").join("governance").join("port-registry.json");
    // Should be:
    // let path = home.join(".local").join("state").join("nabi").join("ports").join("registry.json");
}
```

**Impact**: Rust commands will fail if registry is only in state directory.

---

## 6. MULTI-PORT CONFIGURATION

### "ports" Field Support

The implementation has **partial** support for multiple ports per service:

#### TOML Level (infrastructure.toml)

Currently NOT used in provided examples, but structured to support:

```toml
[services.memchain_mcp_sse]
port = 8001  # Primary port
# Future: could add
# ports = { http = 8001, ws = 8002 }
```

#### Registry Level (registry.json)

Transformation creates separate entries:

```json
{
  "memchain_mcp_sse": {
    "port": 8001,
    "protocol": "http+sse"  // Combined protocol
  }
}
```

#### Rust Level (port.rs)

Renders multi-port services in `list` command:

```rust
if let Some(ports) = &service_spec.ports {
    let ports_str = ports.iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
    println!("  {} {}: ports {}", status, service_name, ports_str.join(", "));
}
```

**Status**: Infrastructure is there, but:
- ❌ Not used in actual TOML configs
- ❌ Not integrated into conflict detection (only checks `port` field)
- ❌ Not tested

---

## 7. RACE CONDITIONS & CONFLICT HANDLING

### Identified Issues

1. **Registry Loading Race** (port.rs line 177)
   - No locking mechanism on registry reads
   - If multiple commands run concurrently, could see stale state
   - Mitigation: Python transformer is single-threaded, Rust commands are read-only

2. **Port Conflict Detection Gaps** (port.rs line 400-476)
   - Doesn't handle "ports" field (HashMap)
   - Only checks single `port` field
   - Could miss conflicts in multi-port services

3. **Drift Detection Window** (validate_ports.py)
   - Snapshot-based (only current moment)
   - No historical tracking
   - `--since` flag exists but not implemented

---

## 8. ARCHITECTURE HIGHLIGHTS

### Strengths

✅ **Clean separation of concerns**
- Python: Transform (TOML → JSON)
- Python: Validate (JSON → Reality)
- Rust: Commands (Display, check, analyze)

✅ **XDG compliant** (Python layer)
- Config in `~/.config/nabi/ports/`
- State in `~/.local/state/nabi/ports/`

✅ **Platform-aware**
- Supports: macos, wsl, rpi, linux
- Per-platform port overrides (e.g., loki: 3100→3101 on WSL)
- Per-platform endpoints (tailscale, localhost)

✅ **Comprehensive validation**
- Port range checks
- Protocol validation
- Cross-platform conflict detection
- Required service health checks
- Drift forensics

✅ **Production ready**
- Error handling with context
- Dry-run safety (shift command)
- Health checks for running services
- Auto-fix command generation

### Gaps

❌ **XDG path bug** in Rust loader
- Points to config dir instead of state dir

❌ **Incomplete multi-port support**
- Structure exists but not fully integrated
- No conflict checking for multi-port services

❌ **Limited HTTP health checks**
- Only TCP connectivity, not HTTP endpoints
- Would need reqwest dependency

❌ **No shift/migration execution**
- Only generates dry-run plans
- Actual port migration not implemented

❌ **No CLI command to trigger transformation**
- Manual `python transform_ports.py` required
- Should be `nabi port rebuild` or similar

---

## 9. DATA FLOW DIAGRAM

```
User Config
    ↓
~/.config/nabi/ports/*.toml ← Human edits
    ↓ transform_ports.py
    ├─ Load TOML files
    ├─ Validate (ranges, protocols, conflicts)
    ├─ Resolve platform overrides
    └─ Write JSON + manifest
    ↓
~/.local/state/nabi/ports/
    ├─ registry.json (14 services × 3 platforms)
    └─ manifest.json (audit trail)
    ↓ port.rs (load_registry)
    ├─ nabi port list --platform wsl
    ├─ nabi port check (vs lsof + docker ps)
    ├─ nabi port cross-platform
    ├─ nabi port drift --forensic
    └─ nabi port fix
    ↓
validate_ports.py (reality check)
    ├─ Compare declared (registry) vs actual (lsof/docker)
    ├─ Report drift (required, warnings, undeclared)
    └─ Generate validation_report_*.json
```

---

## 10. CRITICAL INSIGHTS

### Source of Truth

**TOML files are the source of truth**, not the JSON registry:

1. User edits `~/.config/nabi/ports/infrastructure.toml`
2. Run `python transform_ports.py` (or future `nabi port rebuild`)
3. Generates `~/.local/state/nabi/ports/registry.json`
4. Commands read from JSON (faster, validated)

### Platform Override Resolution

The TOML structure supports intentional drift:

```toml
[services.loki]
port = 3100  # Standard/default

[services.loki.platforms.wsl]
port = 3101  # WSL uses 3101 intentionally (isolation)

[services.grafana.platforms.rpi]
port = 3003  # RPi exposes via 3003:3000 mapping
container_port = 3000
```

The registry and validator correctly resolve these per-platform.

### Manifest as Audit Trail

The manifest.json tracks which TOML files contributed to the registry, enabling:
- Reproducibility ("which configs generated this registry?")
- Orphan detection (TOML deleted but not regenerated?)
- Version control audit trail

---

## Summary Table

| Component | Lang | Lines | Status | Key Issue |
|-----------|------|-------|--------|-----------|
| port.rs (commands) | Rust | 664 | ✅ Full | Registry path XDG bug |
| transform_ports.py | Python | 395 | ✅ Full | No CLI entry point |
| validate_ports.py | Python | 365 | ✅ Full | --since flag not implemented |
| _ranges.toml | TOML | 53 | ✅ Full | N/A |
| *.toml configs | TOML | 200+ | ⚠️ Partial | Multi-port not used |
| registry.json | JSON | 288 | Generated | N/A |
| manifest.json | JSON | 45 | Generated | N/A |

