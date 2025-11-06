# XDG Migration Investigation - Quick Summary
**Date**: 2025-11-05  
**Status**: 70% Complete  
**Severity**: 🔴 CRITICAL - Port registry path resolution fragile

---

## The Problem in 30 Seconds

1. **Rust code is fully XDG-compliant** ✅
2. **Port registry stored in STATE** (`~/.local/state/nabi/`) **but code expects CONFIG** (`~/.config/nabi/`) ❌
3. **NABI_HOME env var masks the problem** - Works now, but fails if unset
4. **"Multi" platform doesn't exist** - Was a planned but never-implemented aggregation config
5. **Multiple backup artifacts scattered** - Partial migration left orphaned files

---

## Critical Issues

| Issue | Evidence | Impact |
|-------|----------|--------|
| **Port registry path mismatch** | Code looks in `~/.config/nabi/governance/` but file is at `~/.local/state/nabi/governance/` | Fails without NABI_HOME env var |
| **Missing XDG fallback logic** | `port.rs:189-209` doesn't check XDG_STATE_HOME | Cross-platform deployments will fail |
| **"Multi" platform artifact** | `.config-state/.consolidation-phase` contains "inventory" | Incomplete state machine left behind |
| **Orphaned migration tools** | Migration scripts exist but didn't complete execution | Inconsistent config/state split |

---

## Root Cause

**Async migration with split responsibilities**:
- Oct 25: Port registry moved to `~/.local/state/nabi/`
- Code still expects `~/.config/nabi/`
- NABI_HOME workaround masks the problem
- Migration never completed full verification/validation

---

## What "Multi in XDG State" Means

**Most likely**: A planned **"multi-platform aggregation config"** that was:
1. Conceptually designed to consolidate shared settings across wsl/macos/rpi
2. Never actually implemented in the port registry
3. Left as state machine artifact in `.config-state/.consolidation-phase`

**Confirmation**: 
```bash
$ jq '.platform_configs | keys' ~/.local/state/nabi/port-registry.json
["wsl", "macos", "rpi"]  # Only these 3, no "multi"
```

---

## Critical Regression Risk

If NABI_HOME is unset anywhere (WSL, RPi, CI/CD):

```bash
$ unset NABI_HOME
$ nabi port list
Error: Registry not found at ~/.config/nabi/governance/port-registry.json
```

The code **doesn't have a proper fallback to XDG_STATE_HOME**.

---

## Quick Fix (Priority 1)

Update `/Users/tryk/nabia/core/nabi-cli/src/commands/port.rs:189-209`:

```rust
fn get_registry_path() -> Result<PathBuf> {
    // Check NABI_HOME first (for backwards compatibility)
    if let Ok(nabi_home) = std::env::var("NABI_HOME") {
        let path = PathBuf::from(nabi_home).join("governance").join("port-registry.json");
        if path.exists() { return Ok(path); }
    }
    
    // Check STATE_HOME (where file actually is!)
    let state_dir = NabiPaths::state_dir()?;
    let path = state_dir.join("governance").join("port-registry.json");
    if path.exists() { return Ok(path); }
    
    // Fallback to CONFIG_HOME (for git-managed scenarios)
    let config_dir = NabiPaths::config_dir()?;
    let path = config_dir.join("governance").join("port-registry.json");
    if path.exists() { return Ok(path); }
    
    anyhow::bail!("Registry not found")
}
```

---

## Evidence Trail

### Files Analyzed
- ✅ `/Users/tryk/nabia/core/nabi-cli/src/paths.rs` - XDG implementation (GOOD)
- ❌ `/Users/tryk/nabia/core/nabi-cli/src/commands/port.rs` - Path resolution (BROKEN)
- ✅ `~/.local/state/nabi/port-registry.json` - Actual registry location
- ❌ `~/.config/nabi/governance/` - Expected but missing
- 📋 `~/.local/state/nabi/nabi_migration_breakages.json` - Detailed migration audit
- 🔧 `~/.config/nabi/.config-state/.consolidation-phase` - Incomplete state artifact

### Directory Structure Mismatch
```
Code expects:     ~/.config/nabi/governance/port-registry.json
Actual location:  ~/.local/state/nabi/governance/port-registry.json
Root cause:       Port registry is RUNTIME DATA (STATE) not CONFIGURATION
```

---

## Full Investigation

See: `XDG_MIGRATION_INVESTIGATION_2025-11-05.md` (9 sections, 400+ lines)

For detailed analysis of:
- XDG path resolution code flow
- State/config separation rationale
- Migration timeline reconstruction
- Backup inventory and recovery options
- Multi-platform validation strategy

---

## Status Summary

**What Works** (8 components):
- ✅ nabi-port.sh (operational)
- ✅ Health check validator
- ✅ Substrate remediation
- ✅ Adapter router
- ✅ Port registry file (valid JSON)
- ✅ Symlink hub navigation

**What's Broken** (4 issues):
- ❌ Port registry path resolution (fragile)
- ❌ nabi-docs.sh bridge (missing)
- ❌ Hook migration incomplete
- ❌ State machine artifact orphaned

**Next Actions**:
1. Fix port registry path resolution (CRITICAL)
2. Restore missing bridges from backups
3. Complete hook migration
4. Validate cross-platform consistency

