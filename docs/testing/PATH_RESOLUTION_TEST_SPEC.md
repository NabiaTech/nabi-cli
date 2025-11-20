# Path Resolution Test Specification

**Version**: 1.0
**Date**: 2025-11-19
**Status**: Regression Prevention Spec

## Executive Summary

### What Happened
A critical path resolution bug caused complete event stream integration failure between nabi-cli (Rust) and federation hooks (Python):

- **Writer**: `nabi events publish` wrote 40,217 events to `~/Library/Application Support/nabi/events/`
- **Reader**: `federation_events_synapse_monitor.py` looked in `~/.local/state/nabi/events/`
- **Result**: Hook found ZERO events despite successful writes

### Why It Matters
This represents a **silent integration failure** - both components worked perfectly in isolation but failed catastrophically when integrated. No errors were logged. The bug was invisible until manual debugging traced the full data flow.

### Root Causes
1. **Platform-specific defaults**: Rust `dirs` crate uses different paths per OS (macOS vs Linux)
2. **Inconsistent abstractions**: Mixed use of `dirs::data_local_dir()`, `NabiPaths`, and hardcoded paths
3. **No XDG compliance**: Environment variables (`XDG_DATA_HOME`) ignored by some code paths
4. **No cross-language validation**: No tests verify Rust and Python resolve to same paths
5. **Silent failures**: Hooks returned empty results without logging path mismatches

## Test Matrix

### Platform × XDG × Storage Type

| Platform | XDG_DATA_HOME | Storage Type | Expected Rust Path | Expected Python Path | Must Match |
|----------|---------------|--------------|-------------------|---------------------|-----------|
| macOS | unset | data | `~/Library/Application Support/nabi/` | `~/.local/share/nabi/` | ❌ MISMATCH |
| macOS | set to `~/.local/share` | data | `~/.local/share/nabi/` | `~/.local/share/nabi/` | ✅ MATCH |
| Linux | unset | data | `~/.local/share/nabi/` | `~/.local/share/nabi/` | ✅ MATCH |
| Linux | set to `/custom` | data | `/custom/nabi/` | `/custom/nabi/` | ✅ MATCH |
| macOS | unset | state | `~/Library/Application Support/nabi/` | `~/.local/state/nabi/` | ❌ MISMATCH |
| macOS | set | state | `$XDG_STATE_HOME/nabi/` | `$XDG_STATE_HOME/nabi/` | ✅ MATCH |
| Linux | unset | cache | `~/.cache/nabi/` | `~/.cache/nabi/` | ✅ MATCH |
| Linux | set | cache | `$XDG_CACHE_HOME/nabi/` | `$XDG_CACHE_HOME/nabi/` | ✅ MATCH |

### Current State Analysis

**Rust Code** (`src/commands/events/mod.rs:204-211`):
```rust
fn get_event_store_path() -> Result<PathBuf> {
    let state_dir = dirs::data_local_dir()  // ⚠️ PLATFORM-SPECIFIC, IGNORES XDG
        .context("Could not determine local data directory")?
        .join("nabi")
        .join("events");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join("event_stream.jsonl"))
}
```

**Python Code** (`federation_events_synapse_monitor.py:128-130`):
```python
def _read_events_from_directory(since_hours: int = 24) -> List[Dict[str, Any]]:
    """Fallback: Read events directly from ~/.local/state/nabi/events/"""
    events = []
    events_dir = Path.home() / ".local" / "state" / "nabi" / "events"  # ⚠️ HARDCODED
```

**Problem**: Complete path mismatch on macOS!

## Test Suite Design

### 1. Unit Tests - Path Resolution

#### Rust Unit Tests (`src/paths.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_data_dir_respects_xdg() {
        // Set XDG environment variable
        env::set_var("XDG_DATA_HOME", "/custom/data");

        let data_dir = NabiPaths::data_dir().unwrap();

        assert_eq!(data_dir, PathBuf::from("/custom/data/nabi"));
        env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn test_data_dir_platform_default() {
        env::remove_var("XDG_DATA_HOME");

        let data_dir = NabiPaths::data_dir().unwrap();

        #[cfg(target_os = "macos")]
        assert!(data_dir.starts_with(dirs::home_dir().unwrap().join(".local/share")));

        #[cfg(target_os = "linux")]
        assert!(data_dir.starts_with(dirs::home_dir().unwrap().join(".local/share")));
    }

    #[test]
    fn test_all_storage_types_consistent() {
        let data = NabiPaths::data_dir().unwrap();
        let state = NabiPaths::state_dir().unwrap();
        let cache = NabiPaths::cache_dir().unwrap();
        let config = NabiPaths::config_dir().unwrap();

        // All should use same base pattern
        assert_eq!(data.parent(), state.parent());
        assert!(data.ends_with("nabi"));
        assert!(state.ends_with("nabi"));
        assert!(cache.ends_with("nabi"));
        assert!(config.ends_with("nabi"));
    }
}
```

#### Python Unit Tests (`tests/test_platform_paths.py`)

```python
import os
import pytest
from pathlib import Path
from platform_paths import get_data_dir, get_state_dir, get_cache_dir, get_config_dir

def test_data_dir_respects_xdg(monkeypatch):
    """Test that XDG_DATA_HOME environment variable is respected"""
    monkeypatch.setenv("XDG_DATA_HOME", "/custom/data")

    data_dir = get_data_dir()

    assert data_dir == Path("/custom/data/nabi")

def test_data_dir_platform_default(monkeypatch):
    """Test platform-specific default when XDG not set"""
    monkeypatch.delenv("XDG_DATA_HOME", raising=False)

    data_dir = get_data_dir()

    # Should always resolve to XDG-compliant path
    assert data_dir == Path.home() / ".local" / "share" / "nabi"

def test_all_storage_types_use_xdg():
    """Verify all storage types follow XDG spec"""
    data = get_data_dir()
    state = get_state_dir()
    cache = get_cache_dir()
    config = get_config_dir()

    assert data.name == "nabi"
    assert state.name == "nabi"
    assert cache.name == "nabi"
    assert config.name == "nabi"
```

### 2. Integration Tests - Cross-Language Consistency

#### Test: Rust-Python Path Agreement

Create `tests/integration/test_path_consistency.rs`:

```rust
use std::process::Command;
use std::env;

#[test]
fn test_rust_python_data_dir_match() {
    // Get Rust path
    let rust_path = NabiPaths::data_dir().unwrap();

    // Get Python path by calling platform_paths.py
    let output = Command::new("python3")
        .arg("-c")
        .arg("from platform_paths import get_data_dir; print(get_data_dir())")
        .output()
        .expect("Failed to execute Python");

    let python_path = String::from_utf8(output.stdout).unwrap().trim().to_string();

    assert_eq!(
        rust_path.to_string_lossy(),
        python_path,
        "Rust and Python must resolve to same data directory"
    );
}

#[test]
fn test_rust_python_paths_match_with_xdg() {
    env::set_var("XDG_DATA_HOME", "/tmp/test_data");
    env::set_var("XDG_STATE_HOME", "/tmp/test_state");
    env::set_var("XDG_CACHE_HOME", "/tmp/test_cache");
    env::set_var("XDG_CONFIG_HOME", "/tmp/test_config");

    // Test data dir
    let rust_data = NabiPaths::data_dir().unwrap();
    let python_data = get_python_path("get_data_dir");
    assert_eq!(rust_data.to_string_lossy(), python_data);

    // Test state dir
    let rust_state = NabiPaths::state_dir().unwrap();
    let python_state = get_python_path("get_state_dir");
    assert_eq!(rust_state.to_string_lossy(), python_state);

    // Cleanup
    env::remove_var("XDG_DATA_HOME");
    env::remove_var("XDG_STATE_HOME");
    env::remove_var("XDG_CACHE_HOME");
    env::remove_var("XDG_CONFIG_HOME");
}

fn get_python_path(func: &str) -> String {
    let output = Command::new("python3")
        .arg("-c")
        .arg(format!("from platform_paths import {}; print({}())", func, func))
        .output()
        .expect("Failed to execute Python");

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
```

### 3. Regression Test - Event Stream Bug

Create `tests/integration/test_event_stream_consistency.rs`:

```rust
#[test]
fn test_event_publish_and_hook_read() {
    // Setup: Clean environment
    let test_event_id = Uuid::new_v4().to_string();

    // Step 1: Publish event via nabi-cli
    let output = Command::new("nabi")
        .args(&["events", "publish"])
        .args(&["--source", "test-integration"])
        .args(&["--message", &format!("integration_test_{}", test_event_id)])
        .output()
        .expect("Failed to publish event");

    assert!(output.status.success(), "Event publish should succeed");

    // Step 2: Simulate hook reading events
    let hook_output = Command::new("python3")
        .arg("~/.local/share/nabi/bin/hooks/federation_events_synapse_monitor.py")
        .env("HOOK_INPUT", r#"{"session_id": "test", "working_dir": "/tmp"}"#)
        .output()
        .expect("Failed to run hook");

    let hook_response: serde_json::Value = serde_json::from_slice(&hook_output.stdout)
        .expect("Hook should return valid JSON");

    // Step 3: Verify hook found the event
    let events_found = hook_response.get("events")
        .and_then(|e| e.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);

    assert!(
        events_found > 0,
        "Hook should find events published by nabi-cli. \
         This test failing indicates writer/reader path mismatch!"
    );
}

#[test]
fn test_event_storage_paths_logged() {
    // Publish with debug logging
    let output = Command::new("nabi")
        .args(&["events", "publish"])
        .args(&["--source", "test-logging"])
        .args(&["--message", "path_logging_test"])
        .env("RUST_LOG", "debug")
        .output()
        .expect("Failed to publish event");

    let stderr = String::from_utf8(output.stderr).unwrap();

    // Verify path is logged
    assert!(
        stderr.contains("event_stream.jsonl") || stderr.contains("Writing event to"),
        "Event publish should log storage path for debugging"
    );
}
```

### 4. Platform Matrix Tests (CI)

Create `.github/workflows/path-resolution-matrix.yml`:

```yaml
name: Path Resolution Matrix

on: [push, pull_request]

jobs:
  test-paths:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        xdg: [set, unset]

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v3

      - name: Setup XDG environment
        if: matrix.xdg == 'set'
        run: |
          echo "XDG_DATA_HOME=$HOME/.local/share" >> $GITHUB_ENV
          echo "XDG_STATE_HOME=$HOME/.local/state" >> $GITHUB_ENV
          echo "XDG_CACHE_HOME=$HOME/.cache" >> $GITHUB_ENV
          echo "XDG_CONFIG_HOME=$HOME/.config" >> $GITHUB_ENV

      - name: Run Rust path tests
        run: cargo test --test path_resolution

      - name: Run Python path tests
        run: python3 -m pytest tests/test_platform_paths.py

      - name: Run cross-language integration tests
        run: cargo test --test path_consistency

      - name: Run event stream regression test
        run: cargo test --test event_stream_consistency
```

## Taskfile Integration

Add to `Taskfile.yaml`:

```yaml
test:path-unit:
  desc: Run path resolution unit tests (Rust + Python)
  cmds:
    - cargo test --lib paths
    - python3 -m pytest tests/test_platform_paths.py -v

test:path-integration:
  desc: Run cross-language path consistency tests
  cmds:
    - cargo test --test path_consistency
    - cargo test --test event_stream_consistency

test:path-all:
  desc: Run complete path resolution test suite
  deps:
    - test:path-unit
    - test:path-integration
  cmds:
    - echo "✓ All path resolution tests passed"

test:path-matrix:
  desc: Run path tests with different XDG configurations
  cmds:
    - task: test:path-all
    - XDG_DATA_HOME=/tmp/test_xdg task test:path-all
    - echo "✓ Path tests passed with and without XDG overrides"

validate:paths:
  desc: Validate current system path configuration
  cmds:
    - |
      echo "=== Current Path Configuration ==="
      echo "Rust data_dir: $(cargo run --quiet -- internal paths data)"
      echo "Python data_dir: $(python3 -c 'from platform_paths import get_data_dir; print(get_data_dir())')"
      echo ""
      echo "Event stream locations:"
      echo "  nabi-cli writes to: $(cargo run --quiet -- internal paths events)"
      echo "  hooks read from: $(python3 -c 'from pathlib import Path; print(Path.home() / \".local\" / \"share\" / \"nabi\" / \"events\")')"
```

## Monitoring Metrics

### Production Monitoring

Add these metrics to track path resolution health:

```toml
# monitoring/metrics.toml

[[metrics]]
name = "event_stream_file_age_seconds"
type = "gauge"
description = "Age of most recent event in event_stream.jsonl"
labels = ["platform", "storage_type"]
alert_threshold = 3600  # Alert if no events in 1 hour

[[metrics]]
name = "hook_event_retrieval_success_rate"
type = "counter"
description = "Success rate of hook event retrieval"
labels = ["hook_name", "source"]
alert_threshold = 0.95  # Alert if success rate < 95%

[[metrics]]
name = "path_resolution_mismatch_total"
type = "counter"
description = "Count of detected path mismatches between components"
labels = ["component_a", "component_b", "storage_type"]
alert_threshold = 1  # Alert on ANY mismatch

[[metrics]]
name = "event_count_divergence"
type = "gauge"
description = "Difference between events written vs events readable"
labels = ["writer", "reader"]
alert_threshold = 100  # Alert if divergence > 100 events
```

### Path Resolution Logging

Add to components:

**Rust** (`src/commands/events/mod.rs`):
```rust
fn handle_publish(...) -> Result<()> {
    let store_path = get_event_store_path()?;

    // Log path for debugging
    log::debug!("Writing event to: {}", store_path.display());

    // Verify Python would read from same location
    let expected_hook_path = NabiPaths::data_dir()?.join("events");
    if store_path.parent() != Some(&expected_hook_path) {
        log::warn!(
            "Path mismatch detected! Writer: {}, Expected reader: {}",
            store_path.display(),
            expected_hook_path.display()
        );
    }

    // ... existing code
}
```

**Python** (`federation_events_synapse_monitor.py`):
```python
def pull_synapse_events(since_hours: int = 24) -> List[Dict[str, Any]]:
    # Get expected path using platform_paths
    from platform_paths import get_data_dir
    expected_path = get_data_dir() / "events" / "event_stream.jsonl"

    # Log for debugging
    logger.debug(f"Reading events from: {expected_path}")

    # Verify this matches what nabi-cli would write
    if not expected_path.exists():
        logger.warning(
            f"Event stream not found at expected path: {expected_path}. "
            "This may indicate path resolution mismatch between Rust and Python."
        )

    # ... existing code
```

## Code Review Checklist

### For All Path-Related Changes

```markdown
## Path Resolution Review Checklist

### Rust Code
- [ ] Uses `NabiPaths` abstraction (not raw `dirs` calls)
- [ ] XDG environment variables respected
- [ ] Path logged at DEBUG level
- [ ] Cross-language path test updated if adding new storage
- [ ] Platform-specific behavior documented

### Python Code
- [ ] Uses `platform_paths.py` (not hardcoded paths like `~/.local/share`)
- [ ] XDG environment variables checked via `os.environ.get()`
- [ ] Path logged for debugging
- [ ] Matches corresponding Rust implementation

### Integration
- [ ] Rust writer + Python reader paths verified to match
- [ ] Integration test added/updated
- [ ] Path mismatch monitoring in place
- [ ] Documentation updated

### Testing
- [ ] Unit tests pass on macOS and Linux
- [ ] Integration tests pass with/without XDG vars set
- [ ] `task test:path-all` passes
- [ ] CI matrix tests pass
```

## Prevention Strategy

### Architectural Requirements

1. **Single Source of Truth**: All path resolution MUST use abstraction layers
   - Rust: `NabiPaths` in `src/paths.rs`
   - Python: `platform_paths.py`

2. **XDG Compliance**: All storage MUST respect XDG environment variables
   - `XDG_DATA_HOME` for persistent data
   - `XDG_STATE_HOME` for runtime state
   - `XDG_CACHE_HOME` for temporary cache
   - `XDG_CONFIG_HOME` for configuration

3. **Cross-Language Consistency**: Rust and Python MUST resolve to same paths
   - Enforce via integration tests
   - Monitor via path mismatch metrics
   - Log all resolved paths at DEBUG level

4. **Platform Transparency**: Code should work identically on macOS/Linux
   - Abstract platform differences in path modules
   - Test on all platforms in CI
   - Document platform-specific behavior

### Developer Workflow

```bash
# Before committing path-related changes:
task test:path-all              # Run all path tests
task validate:paths             # Check current configuration
git commit -m "feat: ..."       # Commit if tests pass

# In CI:
# - Tests run on macOS + Linux matrix
# - Tests run with/without XDG variables
# - Integration tests verify cross-language consistency
```

## Success Criteria

This test specification is successful when:

1. ✅ Developer can run `task test:paths` and verify all paths consistent
2. ✅ CI catches platform-specific path bugs before merge
3. ✅ Monitoring detects when writer/reader paths diverge
4. ✅ Code reviewers have checklist to prevent this class of bug
5. ✅ Future developers understand XDG compliance requirement
6. ✅ Zero production incidents due to path mismatches

## References

### Current Implementation
- Rust paths: `~/nabia/core/nabi-cli/src/paths.rs`
- Python paths: `~/nabia/core/hooks/src/platform_paths.py`
- Event writer: `~/nabia/core/nabi-cli/src/commands/events/mod.rs:204-211`
- Event reader: `~/.local/share/nabi/bin/hooks/federation_events_synapse_monitor.py:128-130`

### Standards
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
- [Rust `dirs` crate documentation](https://docs.rs/dirs/latest/dirs/)
- [Python `platformdirs` package](https://platformdirs.readthedocs.io/)

### Bug Report
- Issue: Path resolution mismatch between nabi-cli and hooks
- Impact: Event stream integration completely broken on macOS
- Root cause: Platform-specific defaults + inconsistent abstractions
- Detection: Manual debugging of "missing events" symptom
- Fix: Use `NabiPaths` consistently + add regression tests
