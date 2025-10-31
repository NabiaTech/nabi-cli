# Nabi CLI - Testing Guide

## Overview

The Nabi CLI includes a comprehensive regression test harness to ensure critical functionality remains stable across refactoring and commander updates. The harness tests:

- Binary availability and executability
- Help/version commands
- Command routing to the correct commanders
- Commander infrastructure
- Python CLI fallback system
- Critical features (like data commander routing)
- Regressions (detecting known issues)

## Quick Start

Run the regression test suite:

```bash
cd ~/.config/nabi/cli
./tests/regression.sh
```

### Verbose Mode

For detailed output about each test:

```bash
./tests/regression.sh --verbose
```

### Fix Mode (Experimental)

Automatically attempt to fix common issues:

```bash
./tests/regression.sh --fix
```

## Understanding Test Results

### Passing Tests ✓

```
✓ nabi binary exists at /Users/tryk/.local/bin/nabi
✓ nabi --help (exit: 0)
✓ Data command routes without crashing
```

### Warnings ⚠

Some warnings are expected during development:

```
⚠ No executable found (expected: /Users/tryk/.config/nabi/commanders/claude/claude)
```

This indicates a commander directory exists but hasn't been fully implemented yet. It will route to the Python CLI fallback instead.

### Failures ✗

Failures indicate real issues that need investigation:

```
✗ Python CLI not found at: ~/nabia/tools/nabi-python
✗ Commanders directory not found: ~/.config/nabi/commanders
```

## Test Categories

### 1. Binary Availability (`test_binary_exists`)

**What it tests**:
- Binary exists at `~/.local/bin/nabi`
- Binary is executable

**Why it matters**:
- The nabi CLI won't function if the binary is missing or not executable
- This is a prerequisite for all other tests

**If it fails**:
```bash
# Rebuild the binary
cd ~/.config/nabi/cli
cargo build --release
cp target/release/nabi ~/.local/bin/
```

### 2. Help & Version (`test_help_commands`)

**What it tests**:
- `nabi --help` works
- `nabi --version` works
- `nabi help` works

**Why it matters**:
- These are the first thing users interact with
- If help breaks, the CLI is unusable

### 3. Command Routing (`test_routing`, `test_commander_routing`)

**What it tests**:
- Commands like `nabi claude --help` route to the correct handler
- Subcommands are parsed correctly
- Commands route to the right commander or fallback

**Why it matters**:
- The entire federation gateway depends on correct routing
- A routing bug could silently execute the wrong command

**Key paths tested**:
```bash
nabi claude --help              # Routes to claude commander
nabi data jsonl validate file   # Routes to data commander
nabi federation agents          # Routes to federation commander
nabi self doctor                # Health check system
```

### 4. Commander Infrastructure (`test_commanders_directory`)

**What it tests**:
- `~/.config/nabi/commanders/` directory exists
- Critical commanders (data, claude, federation) have directories
- Commander executables are found or at least marked with warnings

**Why it matters**:
- Identifies if commanders are properly installed
- Warns about missing implementations (acceptable during development)

### 5. Python CLI Fallback (`test_python_cli_fallback`)

**What it tests**:
- `~/nabia/tools/nabi-python` exists
- Python CLI is executable
- Python CLI can be reached as a fallback

**Why it matters**:
- The data commander uses Python CLI fallback for unimplemented features
- This system prevents hard failures when commanders aren't ready

**If it fails**:
```bash
# Python CLI shim missing - restore from backup or recreate
# See ../README.md for restoration instructions
```

### 6. Critical Features (`test_critical_features`)

**What it tests**:
- Data command with Python CLI routing works
- Commands don't crash the router
- Error messages are coherent

**Why it matters**:
- Detects the specific regression that broke the CLI (premature migration)
- Ensures the fix is working

### 7. Regression Detection (`test_no_regression`)

**What it tests**:
- Specific issue: data commander was routing to errors instead of Python CLI
- Validates the fix: data command shows coherent routing or placeholder messages

**Why it matters**:
- Prevents reintroduction of known bugs during refactoring
- Specifically watches for the premature data commander migration issue

## Adding New Tests

When adding new commands or commanders, add test cases:

```bash
# In tests/regression.sh, add to appropriate test function:

run_test "my new command" "$NABI_BIN my new command" 0
```

Or create a new test function following the pattern:

```bash
test_my_feature() {
    log_section "My Feature Testing"

    log_test "Feature description"
    if some_condition; then
        log_pass "Feature works"
    else
        log_fail "Feature broken"
    fi
}

# Add call to main():
# test_my_feature
```

## Continuous Integration

Run before any commits that modify:

- `src/main.rs` (Rust router changes)
- `src/forge.rs` (Feature flag system)
- Commander implementations
- Deployment changes

```bash
# Before committing
./tests/regression.sh || exit 1
git add .
git commit -m "NOS-XXX: description"
```

## Test Results Interpretation

### All tests pass ✓

```
✓ All tests passed!

Tests run:    12
Tests passed: 23
Tests failed: 0
```

→ CLI is stable. Safe to deploy.

### Some warnings, no failures ⚠

```
⚠ No executable found (expected: .../commanders/claude/claude)

Tests run:    12
Tests passed: 20
Tests failed: 0
```

→ Partial implementation. Acceptable for development. Commands route to fallback (Python CLI).

### Failures ✗

```
✗ Python CLI not found
✗ Commanders directory not found

Tests run:    12
Tests passed: 15
Tests failed: 2
```

→ Infrastructure is broken. **Do not deploy**. Run `--fix` or check manually.

## Performance

The full test suite completes in ~2-3 seconds:

```bash
$ time ./tests/regression.sh
✓ All tests passed!
real  0m2.847s
user  0m1.234s
sys   0m0.512s
```

## Troubleshooting

### Test hangs

If a test seems to hang, it's probably waiting on a subprocess:

```bash
# Run with timeout
timeout 30 ./tests/regression.sh --verbose
```

Check if a commander process is stuck or waiting for input.

### Partial failures

Some individual tests may fail while overall suite passes. This is expected for:

- Health checks (if services are down)
- Commander tests (if implementations are incomplete)
- Integration tests (if Python CLI is in a different state)

### Verbose mode shows command outputs

Run with `--verbose` to debug specific test failures:

```bash
./tests/regression.sh --verbose 2>&1 | grep -A 5 "✗"
```

## History

**Created**: 2025-10-15
**Reason**: CLI broke due to premature data commander migration without proper routing
**Purpose**: Prevent similar regressions through automated testing
**Coverage**: Core routing, commander infrastructure, fallback system, known issues

---

**Related Documents**:
- [README.md](./README.md) - Architecture and implementation phases
- [CLAUDE.md](./CLAUDE.md) - Design philosophy and protocols
- [src/main.rs](./src/main.rs) - Router implementation
