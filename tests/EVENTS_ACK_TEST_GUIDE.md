# Events ACK Test Suite: Quick Start Guide

## Overview

This guide helps you run the comprehensive test suite for `nabi events ack` command. The suite includes 37 tests across three levels.

---

## Quick Start (5 minutes)

### Run All Tests

```bash
cd /Users/tryk/nabia/core/nabi-cli

# Unit tests only (fastest)
cargo test --test events_ack_unit_tests -- --nocapture

# Integration tests (requires --test-threads=1)
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1

# End-to-end tests (full workflow)
bash tests/e2e/agent_c_to_d_test.sh
```

### Expected Results

```
Unit Tests:        21 passed   (~50ms)
Integration Tests: 8 passed    (~400ms)
E2E Tests:         8 passed    (~2s)
=====================================
Total:             37 passed   (~2.5s)
```

---

## Test Files Location

```
/Users/tryk/nabia/core/nabi-cli/
├── tests/
│   ├── events_ack_unit_tests.rs              # 21 unit tests
│   ├── events_ack_integration_tests.rs       # 8 integration tests
│   ├── e2e/
│   │   └── agent_c_to_d_test.sh             # 8 E2E tests
│   ├── EVENTS_ACK_TEST_STRATEGY.md          # Full strategy document
│   └── EVENTS_ACK_TEST_GUIDE.md             # This file
```

---

## Test Descriptions

### Unit Tests (21 tests)

**Vector Clock Operations (7 tests)**
- Creation and initialization
- Increment operations
- Comparison logic (equal, sequential, concurrent)
- Multi-node scenarios

**Causal Ancestry (6 tests)**
- Empty log handling
- Sequential chains
- Multi-node graphs
- Concurrent detection
- Mixed scenarios

**JSONL Operations (5 tests)**
- Single and multiple appends
- Metadata preservation
- Concurrent write safety
- JSON validity

**Event Structure (3 tests)**
- Event creation
- Acknowledgment structure
- Serialization roundtrip

### Integration Tests (8 tests)

- Event creation and acknowledgment flow
- Sequential multi-agent acknowledgments
- Concurrent acknowledgment handling
- Metadata preservation through I/O
- Multiple independent events
- JSONL append ordering
- Empty log handling
- Edge cases (empty vector clocks)

### End-to-End Tests (8 tests)

- Event file creation and verification
- Single acknowledgment workflow
- Multiple sequential acknowledgments
- Vector clock increment in practice
- Metadata preservation in files
- Concurrent acknowledgments with ordering
- Correct file path organization
- Atomic append safety

---

## Running Specific Tests

### Run Only Vector Clock Tests
```bash
cargo test --test events_ack_unit_tests vector_clock_tests -- --nocapture
```

### Run Only Causal Ancestry Tests
```bash
cargo test --test events_ack_unit_tests causal_ancestry_tests -- --nocapture
```

### Run Single Integration Test
```bash
cargo test --test events_ack_integration_tests test_create_event_and_acknowledge -- --nocapture --test-threads=1
```

### Run E2E Test with Artifacts Preserved
```bash
bash tests/e2e/agent_c_to_d_test.sh --preserve
# Artifacts at: /tmp/nabi-events-ack-e2e-test-{PID}/
```

---

## Understanding Test Output

### Successful Unit Test Run
```
running 21 tests
test causal_ancestry_tests::test_causal_ancestors_empty_log ... ok
test causal_ancestry_tests::test_causal_ancestors_first_ack ... ok
test causal_ancestry_tests::test_causal_ancestors_sequential_chain ... ok
...
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

finished in 0.05s
```

### Successful E2E Test Run
```
ℹ Running End-to-End Test Suite
ℹ Setting up test environment...
✓ Test environment ready at /tmp/nabi-events-ack-e2e-test-12345

ℹ Test 1: Create and verify event file
✓ Created test event: event-test-001
✓ Valid event JSON format
✓ Event creation and verification passed

... (more tests)

✓ Tests Passed: 8
===================================================
Test artifacts: /tmp/nabi-events-ack-e2e-test-12345
```

---

## Troubleshooting

### Problem: "error: test failed"

**Check test output** for specific failure:
```bash
# Run with full output
cargo test --test events_ack_unit_tests -- --nocapture --show-output
```

### Problem: Integration test hangs

**Cause**: Multiple tests accessing same files simultaneously.

**Solution**: Always use `--test-threads=1`:
```bash
cargo test --test events_ack_integration_tests -- --test-threads=1
```

### Problem: E2E test says "Event file not created"

**Cause**: File permissions or missing directories.

**Debug**:
```bash
# Run with artifact preservation
bash tests/e2e/agent_c_to_d_test.sh --preserve

# Check what was created
ls -la /tmp/nabi-events-ack-e2e-test-{PID}/events/
```

### Problem: "Invalid JSON" errors

**Cause**: Serialization issue in test fixture.

**Check**:
```bash
# Inspect generated JSONL
cat /tmp/nabi-events-ack-e2e-test-{PID}/events/2025-11-17/*.ack.jsonl | jq '.'

# Should be one valid JSON object per line
```

---

## Test Coverage Verification

### View Test Statistics
```bash
# Count lines in test files
wc -l tests/events_ack_unit_tests.rs
wc -l tests/events_ack_integration_tests.rs
wc -l tests/e2e/agent_c_to_d_test.sh

# Expected: 400+ lines for unit, 300+ for integration, 300+ for e2e
```

### Check Coverage of Core Logic
```bash
# Unit tests exercise vector clock logic (lines ~70-200)
cargo test --test events_ack_unit_tests vector_clock_tests -- --nocapture

# Integration tests exercise file I/O (lines ~250-400)
cargo test --test events_ack_integration_tests integration_tests -- --nocapture --test-threads=1

# E2E tests exercise CLI behavior (lines ~150-300)
bash tests/e2e/agent_c_to_d_test.sh
```

---

## Continuous Integration

### GitHub Actions Setup

Create `.github/workflows/test-events-ack.yml`:

```yaml
name: Events ACK Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - uses: dtolnay/rust-toolchain@stable

      - name: Run unit tests
        run: |
          cd nabia/core/nabi-cli
          cargo test --test events_ack_unit_tests -- --nocapture

      - name: Run integration tests
        run: |
          cd nabia/core/nabi-cli
          cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1

      - name: Run E2E tests
        run: |
          cd nabia/core/nabi-cli
          bash tests/e2e/agent_c_to_d_test.sh
```

---

## Performance Benchmarks

**Target Execution Times**:

| Test Suite | Expected Time | Acceptable Range |
|-----------|---|---|
| Unit tests | 50ms | < 100ms |
| Integration tests | 400ms | < 500ms per test |
| E2E tests | 2s | < 3s total |
| Full suite | 2.5s | < 4s |

**Monitor with**:
```bash
time cargo test --test events_ack_unit_tests -- --nocapture
```

---

## Development Workflow

### When Adding New Tests

1. **Decide test level**:
   - New algorithm? → Unit test
   - New file operation? → Integration test
   - New user workflow? → E2E test

2. **Write test**:
   ```rust
   #[test]
   fn test_new_feature() {
       // Setup
       // Execute
       // Assert
   }
   ```

3. **Run specific test**:
   ```bash
   cargo test test_new_feature -- --nocapture
   ```

4. **Run full suite** to check for regressions:
   ```bash
   cargo test --test events_ack_unit_tests -- --nocapture
   cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
   bash tests/e2e/agent_c_to_d_test.sh
   ```

### When Modifying Core Logic

1. Run tests before modification (establish baseline)
2. Make changes
3. Run related test subset
4. Run full suite
5. Verify no performance regression

---

## Advanced: Custom Test Scenarios

### Create Custom Test Event

```bash
# Create events directory
mkdir -p /tmp/test-events/$(date +%Y-%m-%d)

# Create test event
cat > /tmp/test-events/$(date +%Y-%m-%d)/custom-event.json <<'EOF'
{
  "id": "custom-event",
  "source": "test-source",
  "message": "Custom test event",
  "timestamp": "2025-11-17T12:00:00Z",
  "vector_clock": {
    "macos": 10,
    "rpi": 5
  }
}
EOF
```

### Run Test Against Custom Event

```bash
# Modify integration test to use custom path
# Then run test with custom environment
EVENTS_DIR=/tmp/test-events cargo test --test events_ack_integration_tests test_create_event_and_acknowledge -- --nocapture --test-threads=1
```

---

## Getting Help

### Check Full Strategy Document
```bash
cat /Users/tryk/nabia/core/nabi-cli/tests/EVENTS_ACK_TEST_STRATEGY.md
```

### View Test Source Code
```bash
# Unit tests structure
head -100 /Users/tryk/nabia/core/nabi-cli/tests/events_ack_unit_tests.rs

# Integration tests structure
head -100 /Users/tryk/nabia/core/nabi-cli/tests/events_ack_integration_tests.rs

# E2E test structure
head -150 /Users/tryk/nabia/core/nabi-cli/tests/e2e/agent_c_to_d_test.sh
```

### Check Test Dependencies
```bash
grep -E "use |serde|tempfile" /Users/tryk/nabia/core/nabi-cli/tests/events_ack_unit_tests.rs
```

---

## Quick Reference: Test Commands

```bash
# Core commands
cargo test --test events_ack_unit_tests -- --nocapture
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
bash tests/e2e/agent_c_to_d_test.sh

# Single test
cargo test test_vector_clock_increment -- --nocapture

# With artifacts
bash tests/e2e/agent_c_to_d_test.sh --preserve

# Full output
cargo test --test events_ack_unit_tests -- --nocapture --show-output

# Specific module
cargo test causal_ancestry_tests -- --nocapture

# Ignore slow tests
cargo test --test events_ack_unit_tests -- --nocapture --skip slow
```

---

**Ready to test? Start with:** `cargo test --test events_ack_unit_tests -- --nocapture`
