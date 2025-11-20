# XDG Path Resolution Regression Tests

**Created**: 2025-11-19
**Bug Prevented**: Event stream writer/reader path mismatch (40K events invisible to hooks)

## Quick Start

```bash
# Run all regression tests
task test:xdg:all

# Quick check (unit tests only)
task test:xdg:quick

# Validate codebase for violations
task validate:xdg
```

## What These Tests Prevent

### The Bug
**Symptoms**: Hooks reported ZERO events despite 40K+ events being successfully published

**Root Cause**:
- **Rust (writer)**: Used `dirs::data_local_dir()` → `~/Library/Application Support/nabi/events/` (macOS)
- **Python (reader)**: Hardcoded `~/.local/share/nabi/events/` → XDG default
- **Result**: Complete path mismatch → reader found nothing

**Impact**: Cross-session coordination completely broken on macOS

### The Fix
- **Rust**: Use `NabiPaths::data_dir()` (respects XDG_DATA_HOME)
- **Python**: Use `platform_paths.get_data_dir()` (respects XDG_DATA_HOME)
- **Both**: Check environment variable first, use XDG default second

## Test Coverage

### Rust Tests (`tests/path_resolution_regression.rs`)

**Unit Tests**:
- ✅ `test_data_dir_respects_xdg_data_home` - XDG_DATA_HOME honored
- ✅ `test_data_dir_uses_xdg_default_when_env_not_set` - Default is XDG-compliant
- ✅ `test_state_dir_respects_xdg_state_home` - XDG_STATE_HOME honored
- ✅ `test_cache_dir_respects_xdg_cache_home` - XDG_CACHE_HOME honored
- ✅ `test_config_dir_respects_xdg_config_home` - XDG_CONFIG_HOME honored
- ✅ `test_event_store_uses_nabi_paths` - Events use abstraction
- ✅ `test_all_storage_types_use_xdg_pattern` - Consistency across all types
- ✅ `test_macos_does_not_use_library_application_support` - macOS-specific bug check
- ✅ `test_path_consistency_across_modules` - All modules use same paths

**Integration Tests** (`tests/integration/test_event_stream_cross_language.rs`):
- ✅ `test_rust_publish_python_read_consistency` - **The exact bug scenario**
- ✅ `test_event_stream_file_location_matches` - File written to correct location
- ✅ `test_multiple_xdg_vars_respected` - All XDG variables work

### Python Tests (`../hooks/tests/test_path_xdg_regression.py`)

**Unit Tests**:
- ✅ `test_data_dir_respects_xdg_data_home` - Python side XDG compliance
- ✅ `test_data_dir_uses_xdg_default_when_not_set` - Default matches Rust
- ✅ `test_state_dir_respects_xdg_state_home` - State directory compliance
- ✅ `test_cache_dir_respects_xdg_cache_home` - Cache directory compliance
- ✅ `test_config_dir_respects_xdg_config_home` - Config directory compliance
- ✅ `test_no_hardcoded_local_share_in_hooks` - Static code analysis
- ✅ `test_macos_does_not_use_application_support_with_xdg` - macOS override

**Integration Tests**:
- ✅ `test_publish_and_hook_read_same_location` - **End-to-end verification**
- ✅ `test_rust_python_data_dir_match` - Cross-language consistency

## Running the Tests

### Individual Test Suites

```bash
# Rust unit tests
task test:xdg:rust

# Rust integration tests
task test:xdg:rust:integration

# Python unit tests
task test:xdg:python

# Python integration tests (requires nabi binary installed)
task test:xdg:python:integration
```

### CI/CD Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: XDG Path Regression Tests
  run: |
    task test:xdg:quick  # Fast unit tests

- name: XDG Integration Tests
  run: |
    task install  # Install nabi binary
    task test:xdg:rust:integration
    task test:xdg:python:integration
```

### Pre-commit Hook

Add to `.pre-commit-config.yaml`:

```yaml
- repo: local
  hooks:
    - id: xdg-compliance
      name: Validate XDG path compliance
      entry: task validate:xdg
      language: system
      pass_filenames: false
```

## Test Failure Scenarios

### Failure: "Writer and reader using different paths"

```
REGRESSION: Event was published but pull returned empty. \
This means writer and reader are using different paths!
```

**Diagnosis**: Check that both Rust and Python modules:
1. Import path abstraction (`NabiPaths` / `platform_paths`)
2. Use abstraction consistently (not raw `dirs` or hardcoded paths)
3. Both respect same XDG environment variables

**Fix**:
```rust
// ❌ WRONG
let path = dirs::data_local_dir().unwrap().join("nabi");

// ✅ CORRECT
use crate::paths::NabiPaths;
let path = NabiPaths::data_dir()?;
```

```python
# ❌ WRONG
path = Path.home() / ".local" / "share" / "nabi"

# ✅ CORRECT
from platform_paths import get_data_dir
path = get_data_dir()
```

### Failure: "On macOS, must not use Application Support"

```
On macOS, NabiPaths must respect XDG_DATA_HOME, not use Library/Application Support.
```

**Diagnosis**: Code is using platform-specific default instead of XDG override

**Fix**: Ensure XDG environment variable is checked FIRST, before platform defaults

## Static Analysis

The `validate:xdg` task performs static analysis to catch violations:

```bash
$ task validate:xdg

=== Checking for XDG violations ===
src/commands/events/mod.rs:205:    let state_dir = dirs::data_local_dir()
❌ Found raw dirs usage! Use NabiPaths instead

src/hooks/user_prompt_submit.py:86:    ack_file = Path.home() / ".local/state/nabi"
❌ Found hardcoded paths in Python! Use platform_paths instead
```

## Architecture Requirements

### Rust Modules

**MUST USE**:
```rust
use crate::paths::NabiPaths;

let data_dir = NabiPaths::data_dir()?;
let state_dir = NabiPaths::state_dir()?;
let cache_dir = NabiPaths::cache_dir()?;
let config_dir = NabiPaths::config_dir()?;
```

**NEVER USE**:
```rust
❌ dirs::data_local_dir()
❌ dirs::data_dir()
❌ dirs::state_dir()
❌ dirs::cache_dir()
❌ dirs::config_dir()
```

**Exception**: Only `paths.rs` implementation should use `dirs` crate directly

### Python Modules

**MUST USE**:
```python
from platform_paths import (
    get_data_dir,
    get_state_dir,
    get_cache_dir,
    get_config_dir
)

data_dir = get_data_dir() / "subdir"
```

**NEVER USE**:
```python
❌ Path.home() / ".local/share/nabi"
❌ Path.home() / ".local/state/nabi"
❌ os.path.expanduser("~/.local/share/nabi")
```

**Exception**: Only `platform_paths.py` implementation should construct base paths

## Monitoring in Production

Add these metrics to catch path mismatches in production:

```yaml
metrics:
  - name: event_stream_file_age
    type: gauge
    description: Age of most recent event in stream
    alert: > 1 hour without events (may indicate path mismatch)

  - name: hook_event_retrieval_rate
    type: counter
    description: Success rate of hook event queries
    alert: < 95% success rate (may indicate path issues)

  - name: event_count_divergence
    type: gauge
    description: Difference between published count vs retrieved count
    alert: divergence > 100 events (path mismatch detected)
```

## Success Criteria

These tests are successful when:

1. ✅ All unit tests pass on macOS and Linux
2. ✅ Integration tests verify cross-language consistency
3. ✅ Static analysis finds no raw `dirs::` usage
4. ✅ Static analysis finds no hardcoded `Path.home() / ".local"` usage
5. ✅ Pre-commit hook catches new violations
6. ✅ CI fails on XDG compliance violations

## References

- **Bug Report**: PATH_RESOLUTION_TEST_SPEC.md
- **Flow Trace**: DOCUMENT_DETECTION_FLOW_TRACE.md
- **XDG Spec**: https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
- **Rust paths.rs**: `src/paths.rs`
- **Python platform_paths.py**: `../hooks/src/platform_paths.py`

## Troubleshooting

### Tests fail with "could not find nabi binary"

**Solution**: Run `task install` to install the nabi binary before integration tests

### Tests pass locally but fail in CI

**Solution**: Ensure CI sets `XDG_DATA_HOME` consistently or tests will use different paths

### Python tests skip integration tests

**Solution**: Integration tests marked with `@pytest.mark.integration` - run with `-m integration` flag

## Last Updated

- **Date**: 2025-11-19
- **Bug**: Event stream path mismatch (40K events invisible)
- **Fix**: Use NabiPaths/platform_paths consistently
- **Tests**: Comprehensive coverage of Rust + Python + integration
