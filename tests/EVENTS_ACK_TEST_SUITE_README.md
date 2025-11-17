# Events ACK Test Suite

**Comprehensive Test Suite for `nabi events ack` Command**

## Executive Summary

A production-grade test suite with 37 tests across three levels ensuring robust acknowledgment functionality with vector clock tracking and causal ancestry computation.

**Test Coverage**:
- 21 Unit Tests (vector clocks, ancestry, file operations)
- 8 Integration Tests (complete workflows)
- 8 End-to-End Tests (user-facing scenarios)

**Status**: Ready for implementation | Last Updated: Nov 17, 2025

---

## What Gets Tested

### Core Functionality

1. **Vector Clock Operations** (7 tests)
   - Clock initialization and manipulation
   - Sequential ordering detection (A happened before B)
   - Concurrent event detection (independent agents)
   - Multi-node graph traversal

2. **Causal Ancestry Computation** (6 tests)
   - Identifying all vector clocks that causally precede a target
   - Empty log handling (first acknowledgment)
   - Complex causal chains (3+ agents in sequence)
   - Mixed sequential and concurrent scenarios

3. **Atomic JSONL Operations** (5 tests)
   - Single and bulk append operations
   - Metadata serialization roundtrip
   - Concurrent write safety (5+ simultaneous writes)
   - No partial writes or truncation

4. **End-to-End Workflows** (16 tests across integration + E2E)
   - Event creation and storage
   - Acknowledgment generation
   - JSONL log persistence
   - User CLI interactions

---

## Quick Start

```bash
cd /Users/tryk/nabia/core/nabi-cli

# Run all unit tests (fastest feedback)
cargo test --test events_ack_unit_tests -- --nocapture

# Run integration tests (single-threaded to avoid conflicts)
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1

# Run end-to-end tests (full workflow simulation)
bash tests/e2e/agent_c_to_d_test.sh

# Run everything
./run_all_tests.sh  # (script provided below)
```

**Expected time**: ~2.5 seconds total

---

## Files in This Suite

### Test Implementation Files

| File | Tests | Purpose |
|------|-------|---------|
| `events_ack_unit_tests.rs` | 21 | Core algorithms without file I/O |
| `events_ack_integration_tests.rs` | 8 | Complete workflows with real files |
| `e2e/agent_c_to_d_test.sh` | 8 | User-facing CLI scenarios |

### Documentation Files

| File | Purpose |
|------|---------|
| `EVENTS_ACK_TEST_STRATEGY.md` | Comprehensive strategy document (full context) |
| `EVENTS_ACK_TEST_GUIDE.md` | Quick reference guide (execution how-to) |
| `EVENTS_ACK_TEST_SUITE_README.md` | This file (overview) |

---

## Test Breakdown by Category

### Unit Tests (21 tests) - `events_ack_unit_tests.rs`

Validate algorithms in isolation.

```
Vector Clock Tests (7):
  ✓ Creation and initialization
  ✓ Increment operations
  ✓ Equal clock detection
  ✓ Sequential ordering (A < B)
  ✓ Concurrent detection (no ordering)
  ✓ Multi-node sequential chains
  ✓ Multi-node concurrent graphs

Causal Ancestry Tests (6):
  ✓ Empty log (no predecessors)
  ✓ Single predecessor
  ✓ Full sequential chain
  ✓ Multi-node ordering
  ✓ Concurrent filtering
  ✓ Mixed concurrent + sequential

JSONL Operations Tests (5):
  ✓ Single line append
  ✓ Multiple appends (ordering preserved)
  ✓ Metadata in JSONL
  ✓ Concurrent write safety
  ✓ JSON parsability

Event Structure Tests (3):
  ✓ Event creation
  ✓ Acknowledgment creation
  ✓ Serialization roundtrip
```

**Run**: `cargo test --test events_ack_unit_tests -- --nocapture`

### Integration Tests (8 tests) - `events_ack_integration_tests.rs`

Validate complete workflows with file I/O.

```
✓ Event creation and single acknowledgment
✓ Sequential multi-agent acknowledgments (3 agents)
✓ Concurrent acknowledgments (independent agents)
✓ Metadata preservation through I/O
✓ Multiple independent events (isolation)
✓ JSONL append ordering under load (5 rapid appends)
✓ Empty acknowledgment log handling
✓ Edge case: empty event vector clocks
```

**Run**: `cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1`

### End-to-End Tests (8 tests) - `tests/e2e/agent_c_to_d_test.sh`

Simulate real CLI usage.

```
✓ Event file creation and JSON validation
✓ Single acknowledgment workflow
✓ Multiple sequential acknowledgments
✓ Vector clock increment in practice
✓ Metadata preservation in files
✓ Concurrent acknowledgments with ordering (5 rapid acks)
✓ Correct file path organization by date
✓ Atomic append safety (10 sequential writes)
```

**Run**: `bash tests/e2e/agent_c_to_d_test.sh`

---

## Key Test Scenarios

### Scenario 1: Single Agent Acknowledging

```
Event created:     VC = {macos: 10}
Agent acknowledges: VC becomes {macos: 11}
Ancestors:          [] (first ack, no predecessors)
Result:             JSON appended to JSONL
```

### Scenario 2: Sequential Multi-Agent Acknowledgment

```
Event:              VC = {macos: 5}
Agent 1 (macos):    VC = {macos: 6}, ancestors = []
Agent 2 (rpi):      VC = {rpi: 1, macos: 5}, ancestors = [ack-001]
Agent 3 (wsl):      VC = {wsl: 1, macos: 5, rpi: 1}, ancestors = [ack-001, ack-002]
Result:             3 entries in JSONL, causal chain preserved
```

### Scenario 3: Concurrent Acknowledgment

```
Event:              VC = {macos: 5, rpi: 3}
Agent A (macos):    VC = {macos: 6, rpi: 3}, ancestors = []
Agent B (wsl):      VC = {wsl: 1, macos: 5, rpi: 3}, ancestors = [] (concurrent)
Result:             2 entries, A and B are independent (not causally ordered)
```

---

## Vector Clock Primer

Understanding vector clocks is key to these tests.

**What is a Vector Clock?**

A vector clock is a map of node IDs to counters that tracks causal ordering of events.

```json
{
  "macos": 10,
  "rpi": 5,
  "wsl": 2
}
```

**Key Rules**:

1. **Increment**: When a node acts, it increments its own counter only
2. **Dominance**: Clock A dominates B if:
   - A's value >= B's value for ALL nodes
   - A's value > B's value for AT LEAST ONE node

3. **Causality**: If A dominates B, then A causally precedes B (A happened before B)

4. **Concurrency**: If neither dominates, events are concurrent (no ordering)

**Example**:
```
A = {macos: 6, rpi: 3}
B = {macos: 6, rpi: 4}

All of A's values <= B's values ✓
At least one > (rpi: 3 < 4) ✓
Therefore: A < B (A causally precedes B)
```

---

## Test Execution Workflow

### Development Workflow

```
1. Write test code in appropriate test file
2. Run specific test: cargo test test_name -- --nocapture
3. Debug and iterate
4. Run full test suite for that level
5. Run all three levels before commit
```

### CI/CD Workflow

```
1. On push/PR, GitHub Actions runs:
   - Unit tests (fast feedback)
   - Integration tests
   - E2E tests
2. All 37 tests must pass
3. Coverage report generated
4. Merge only if all green
```

### Debugging Workflow

```
# If test fails:
1. Run with --show-output for full context
2. If file-related, preserve artifacts: bash tests/e2e/... --preserve
3. Inspect JSONL files: jq '.' /tmp/nabi-events-ack-e2e-test-{PID}/...
4. Check vector clock values in test output
5. Review assertion message for expected vs actual
```

---

## Performance Targets

| Test Suite | Expected | Acceptable |
|-----------|----------|-----------|
| Unit (21 tests) | 50ms | < 100ms |
| Integration (8 tests) | 400ms | < 500ms each |
| E2E (8 tests) | 2s | < 3s total |
| **All 37 tests** | **2.5s** | **< 4s** |

Monitor with:
```bash
time cargo test --test events_ack_unit_tests -- --nocapture
```

---

## What's Covered vs. Not Covered

### What IS Covered

- ✅ Vector clock comparison logic
- ✅ Causal ancestry computation
- ✅ JSONL append atomicity
- ✅ Metadata serialization
- ✅ File I/O and storage
- ✅ Event and acknowledgment structures
- ✅ Multi-agent scenarios
- ✅ Concurrent operations
- ✅ Edge cases (empty logs, empty clocks)

### What Needs Command Implementation

These tests assume the command exists. When implementing `nabi events ack`:
- Parse CLI arguments
- Load event from disk
- Load existing acknowledgments
- Increment vector clock
- Compute causal ancestors
- Append to JSONL atomically
- Output JSON to stdout

The tests validate the *algorithms* not the CLI parsing.

---

## Integration with Aether

The test suite uses algorithms from `~/nabia/platform/aether/src/nabia/aether/reconcile.py`:

```python
def compare_vector_clocks(clock_a, clock_b) -> Optional[str]:
    """Compare vector clocks for causal ordering"""
    # Returns "a" if A < B, "b" if B < A, None if concurrent

def compute_causal_ancestors(target_clock, all_clocks) -> list:
    """Find all clocks that causally precede target"""
    # Returns indices of ancestral clocks
```

Our Rust tests replicate this logic to ensure correctness before integration.

---

## Expected Output Examples

### Successful Unit Test Run

```
running 21 tests
test causal_ancestry_tests::test_causal_ancestors_empty_log ... ok
test causal_ancestry_tests::test_causal_ancestors_first_ack ... ok
test causal_ancestry_tests::test_causal_ancestors_sequential_chain ... ok
test causal_ancestry_tests::test_causal_ancestors_multi_node ... ok
test causal_ancestry_tests::test_causal_ancestors_concurrent ... ok
test causal_ancestry_tests::test_causal_ancestors_mixed_concurrent_and_sequential ... ok
test event_structure_tests::test_acknowledgment_creation ... ok
test event_structure_tests::test_acknowledgment_serialization ... ok
test event_structure_tests::test_event_creation ... ok
test jsonl_atomicity_tests::test_jsonl_concurrent_writes_safe ... ok
test jsonl_atomicity_tests::test_jsonl_multiple_appends ... ok
test jsonl_atomicity_tests::test_jsonl_single_append ... ok
test jsonl_atomicity_tests::test_jsonl_with_metadata ... ok
test vector_clock_tests::test_vector_clock_comparison_concurrent ... ok
test vector_clock_tests::test_vector_clock_comparison_equal ... ok
test vector_clock_tests::test_vector_clock_comparison_multi_node_concurrent ... ok
test vector_clock_tests::test_vector_clock_comparison_multi_node_sequential ... ok
test vector_clock_tests::test_vector_clock_comparison_sequential ... ok
test vector_clock_tests::test_vector_clock_creation ... ok
test vector_clock_tests::test_vector_clock_from_map ... ok
test vector_clock_tests::test_vector_clock_increment ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

### Successful E2E Test Run

```
ℹ =====================================================
ℹ Running End-to-End Test Suite
ℹ =====================================================

ℹ Setting up test environment...
✓ Test environment ready at /tmp/nabi-events-ack-e2e-test-98765

ℹ Test 1: Create and verify event file
✓ Created test event: event-test-001
✓ Valid event JSON format
✓ Event creation and verification passed

ℹ Test 2: Acknowledge single event
✓ Appended acknowledgment: ack-event-test-002-1234567890
✓ Valid acknowledgment JSON format
✓ Single acknowledgment passed

... (6 more tests)

ℹ =====================================================
ℹ Test Results Summary
ℹ =====================================================
✓ Tests Passed: 8
Test artifacts: /tmp/nabi-events-ack-e2e-test-98765
```

---

## Troubleshooting

### "test failed"

Run with verbose output:
```bash
cargo test --test events_ack_unit_tests -- --nocapture --show-output
```

Check the specific assertion that failed and review test logic.

### Integration test hangs or conflicts

**Must use single-threaded mode**:
```bash
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
```

### E2E test "Event file not created"

Preserve artifacts for inspection:
```bash
bash tests/e2e/agent_c_to_d_test.sh --preserve
ls /tmp/nabi-events-ack-e2e-test-{PID}/events/
```

### JSON parsing errors

Inspect generated JSONL:
```bash
cat /tmp/nabi-events-ack-e2e-test-{PID}/events/*/event-test-*.ack.jsonl | jq '.'
```

Should be one valid JSON object per line.

---

## Maintenance

### When to Update Tests

- **New algorithm logic**: Add unit test
- **New file operation**: Add integration test
- **New user workflow**: Add E2E test
- **Bug discovered**: Add regression test

### When to Review

- Quarterly review of test effectiveness
- After major refactoring
- Before major release

### Performance Regression

If tests suddenly slow down:
```bash
# Measure baseline
time cargo test --test events_ack_unit_tests -- --nocapture

# Identify slow test
cargo test --test events_ack_unit_tests -- --nocapture --test-threads=1
```

---

## Success Metrics

- ✅ 37 tests passing (100%)
- ✅ All edge cases covered
- ✅ Zero flaky tests (consistent pass rate)
- ✅ Execution time < 4s for full suite
- ✅ Code coverage > 95%

---

## Next Steps

### For Implementation

1. Implement `nabi events ack` command in `src/commands/events.rs`
2. Add Ack struct and related types
3. Integrate Python reconcile bridge
4. Run unit tests first (algorithms)
5. Run integration tests (file I/O)
6. Run E2E tests (CLI behavior)
7. All 37 tests must pass before merge

### For Team

1. Review test strategy document
2. Run tests locally before coding
3. Add new tests when adding features
4. Monitor CI/CD test results
5. Maintain performance targets

---

## Document References

- **Full Strategy**: `EVENTS_ACK_TEST_STRATEGY.md` (comprehensive)
- **Quick Guide**: `EVENTS_ACK_TEST_GUIDE.md` (execution reference)
- **This Overview**: `EVENTS_ACK_TEST_SUITE_README.md` (what you're reading)

---

## Quick Command Reference

```bash
# Run all tests
cargo test --test events_ack_unit_tests -- --nocapture && \
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1 && \
bash tests/e2e/agent_c_to_d_test.sh

# Run specific level
cargo test --test events_ack_unit_tests -- --nocapture
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
bash tests/e2e/agent_c_to_d_test.sh

# Run with artifacts preserved
bash tests/e2e/agent_c_to_d_test.sh --preserve

# Debug specific test
cargo test vector_clock_increment -- --nocapture --show-output

# Check performance
time cargo test --test events_ack_unit_tests -- --nocapture

# View test output
cat /tmp/nabi-events-ack-e2e-test-{PID}/events/*/event-*.ack.jsonl | jq '.'
```

---

**Ready to test? Start with:**

```bash
cd /Users/tryk/nabia/core/nabi-cli
cargo test --test events_ack_unit_tests -- --nocapture
```

**Questions? See:**
- Full strategy: `EVENTS_ACK_TEST_STRATEGY.md`
- Quick guide: `EVENTS_ACK_TEST_GUIDE.md`
