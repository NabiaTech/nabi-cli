# Test Strategy: `nabi events ack` Command

## Document Version
- **Version**: 1.0
- **Last Updated**: November 17, 2025
- **Status**: Ready for Implementation
- **Audience**: Test Engineers, Developers, QA

---

## Executive Summary

This document defines the comprehensive test strategy for the `nabi events ack` command, which enables federation agents to acknowledge events with automatic vector clock tracking and causal ancestry computation.

The strategy is organized across three test levels:
- **Unit Tests** (21 tests): Vector clock operations, causal ancestry, JSONL atomicity
- **Integration Tests** (8 tests): End-to-end workflows with real file operations
- **End-to-End Tests** (8 tests): Full Agent C → D acknowledgment flow

**Total Coverage**: 37 tests ensuring production-grade reliability.

---

## 1. Requirements Analysis

### Command Specification

```
nabi events ack <event_id> [--metadata <json>] [--json]
```

**Core Responsibilities**:
1. Load event from `~/.local/state/nabi/events/{date}/{event_id}.json`
2. Load existing acknowledgments from `{event_id}.ack.jsonl`
3. Increment agent's vector clock counter
4. Compute causal ancestors via vector clock comparison
5. Append acknowledgment to JSONL atomically
6. Output JSON to stdout (if `--json` flag)

### Key Attributes

| Attribute | Specification |
|-----------|----------------|
| **Vector Clock Format** | `{node_id: counter}` map |
| **Increment Rule** | Local node counter += 1 |
| **Causal Ancestor Detection** | Vector clock dominance comparison |
| **Storage Format** | One JSON-serialized ack per JSONL line |
| **Atomicity Guarantee** | File append operations never truncate |
| **Metadata Handling** | Optional JSON field preserved through serialization |

---

## 2. Test Levels and Coverage

### 2.1 Unit Tests (21 tests)

**File**: `tests/events_ack_unit_tests.rs`

**Objective**: Validate core algorithms in isolation without file I/O.

#### Vector Clock Operations (7 tests)

These tests verify the mathematical correctness of vector clock operations.

| Test | Description | Validates |
|------|-------------|-----------|
| `test_vector_clock_creation` | Initialize empty clock | Clock initialization |
| `test_vector_clock_from_map` | Create from HashMap | HashMap conversion |
| `test_vector_clock_increment` | Single and multiple increments | Counter increment logic |
| `test_vector_clock_comparison_equal` | Equal clocks return None | Equality detection |
| `test_vector_clock_comparison_sequential` | A < B detected | Sequential ordering |
| `test_vector_clock_comparison_concurrent` | Concurrent detected | Concurrency detection |
| `test_vector_clock_comparison_multi_node_sequential` | Multi-node sequential | Complex causal chains |

**Coverage Goal**: 100% of vector clock comparison logic.

**Example Test Case**:
```rust
#[test]
fn test_vector_clock_comparison_sequential() {
    // A: {macos: 5}, B: {macos: 6}
    // Expected: A < B (A causally precedes B)
    assert_eq!(vc_a.compare(&vc_b), Some(true));
}
```

#### Causal Ancestry Computation (6 tests)

These tests verify correct identification of causal predecessors.

| Test | Description | Validates |
|------|-------------|-----------|
| `test_causal_ancestors_empty_log` | No predecessors when log empty | Base case |
| `test_causal_ancestors_first_ack` | First ack has single predecessor | Sequential case |
| `test_causal_ancestors_sequential_chain` | Full causal chain identified | Chain detection |
| `test_causal_ancestors_multi_node` | Multi-node ordering | Complex graphs |
| `test_causal_ancestors_concurrent` | Concurrent events not included | Concurrency filtering |
| `test_causal_ancestors_mixed_concurrent_and_sequential` | Mixed scenarios | Realistic graphs |

**Coverage Goal**: 100% of ancestry computation logic.

#### Atomic JSONL Operations (5 tests)

These tests verify file append safety and JSON preservation.

| Test | Description | Validates |
|------|-------------|-----------|
| `test_jsonl_single_append` | One line append succeeds | Basic append |
| `test_jsonl_multiple_appends` | Multiple appends preserve order | Append ordering |
| `test_jsonl_with_metadata` | Metadata preserved in JSONL | Serialization integrity |
| `test_jsonl_concurrent_writes_safe` | 5 rapid writes all present | Concurrency safety |
| *(Implicit atomicity test)* | No partial writes | File integrity |

**Coverage Goal**: 100% of file operations.

#### Event Structure (3 tests)

Basic serialization and deserialization tests.

| Test | Description | Validates |
|------|-------------|-----------|
| `test_event_creation` | Event initialization | Data structure |
| `test_acknowledgment_creation` | Ack initialization | Data structure |
| `test_acknowledgment_serialization` | Round-trip JSON | Serialization |

---

### 2.2 Integration Tests (8 tests)

**File**: `tests/events_ack_integration_tests.rs`

**Objective**: Validate complete workflows with real file operations.

**Note**: Run with `--test-threads=1` to prevent directory conflicts.

#### Test Cases

| Test | Description | Validates |
|------|-------------|-----------|
| `test_create_event_and_acknowledge` | Full event → ack flow | End-to-end correctness |
| `test_multiple_sequential_acknowledgments` | Three agents ack in sequence | Causal chain in practice |
| `test_concurrent_acknowledgments` | Multiple independent agents | Concurrency handling |
| `test_ack_with_metadata` | Metadata round-trip | Metadata preservation |
| `test_multiple_events` | Three independent events | Event isolation |
| `test_ack_log_append_ordering` | Rapid appends maintain order | Concurrency + ordering |
| `test_empty_ack_log` | Handle missing log file | Error handling |
| `test_ack_with_empty_event_vector_clock` | Edge case handling | Robustness |

**Coverage Goal**: 100% of integration paths.

**Example Workflow**:
```
1. Create event with VC {macos: 5}
2. First agent acknowledges → VC becomes {macos: 6}, ancestors=[]
3. Second agent acknowledges → VC becomes {rpi: 1}, ancestors=[ack-001]
4. Third agent acknowledges → VC becomes {wsl: 1}, ancestors=[ack-001, ack-002]
5. Verify JSONL contains 3 valid entries in causal order
```

---

### 2.3 End-to-End Tests (8 tests)

**File**: `tests/e2e/agent_c_to_d_test.sh`

**Objective**: Validate complete workflow from event publication to storage.

**Execution Model**: Bash script simulating real CLI behavior.

#### Test Cases

| Test | Shell | Validates |
|------|-------|-----------|
| `test_1_create_and_verify_event` | `bash` | Event file creation |
| `test_2_acknowledge_single_event` | `bash` | Single ack workflow |
| `test_3_multiple_sequential_acks` | `bash` | Sequential acks |
| `test_4_vector_clock_increment` | `bash` | VC increment in practice |
| `test_5_metadata_preservation` | `bash` | Metadata survival |
| `test_6_concurrent_acks_ordering` | `bash` | Ordering under load |
| `test_7_ack_log_location` | `bash` | Correct file paths |
| `test_8_atomicity` | `bash` | File append safety |

**Coverage Goal**: 100% of user-facing workflows.

**Test Execution Pattern**:
```bash
# Run all E2E tests
bash tests/e2e/agent_c_to_d_test.sh

# Preserve test artifacts for debugging
bash tests/e2e/agent_c_to_d_test.sh --preserve
```

---

## 3. Test Execution Strategy

### 3.1 Running Tests

**Unit Tests**:
```bash
cd /Users/tryk/nabia/core/nabi-cli
cargo test --test events_ack_unit_tests -- --nocapture
```

**Expected Output**:
```
running 21 tests
test vector_clock_tests::test_vector_clock_creation ... ok
test vector_clock_tests::test_vector_clock_increment ... ok
...
test result: ok. 21 passed; 0 failed; 0 ignored
```

**Integration Tests** (single-threaded):
```bash
cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
```

**Expected Output**:
```
running 8 tests
test integration_tests::test_create_event_and_acknowledge ... ok
...
test result: ok. 8 passed; 0 failed; 0 ignored
```

**End-to-End Tests**:
```bash
bash tests/e2e/agent_c_to_d_test.sh
```

**Expected Output**:
```
ℹ Running End-to-End Test Suite
...
✓ Tests Passed: 8
===================================================
```

### 3.2 CI/CD Integration

**GitHub Actions Workflow**:
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
        run: cargo test --test events_ack_unit_tests -- --nocapture
      - name: Run integration tests
        run: cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
      - name: Run E2E tests
        run: bash tests/e2e/agent_c_to_d_test.sh
```

---

## 4. Test Coverage Metrics

### Overall Coverage

| Test Level | Count | Coverage |
|-----------|-------|----------|
| **Unit** | 21 | Vector clocks, ancestry, JSONL |
| **Integration** | 8 | File I/O, workflows |
| **E2E** | 8 | Full user workflows |
| **Total** | 37 | Comprehensive |

### Coverage by Component

| Component | Tests | Coverage |
|-----------|-------|----------|
| Vector Clock Logic | 7 + 6 = 13 | 100% |
| JSONL Operations | 5 | 100% |
| Event Handling | 3 | 100% |
| File Operations | 8 | 100% |
| User Workflows | 8 | 100% |

---

## 5. Test Data Strategy

### Test Fixtures

**Location**: `tests/fixtures/`

**Required Fixtures**:
```
fixtures/
├── test-event-001.json          # Sample event with VC {macos: 10}
├── test-event-002.json          # Multi-node event
├── test-event-003.ack.jsonl     # Sample acknowledgment log
└── concurrent-ack-scenario.json # Concurrent event spec
```

### Test Data Properties

**Vector Clock Samples**:
- Empty: `{}`
- Single node: `{macos: 1}`
- Multi-node: `{macos: 5, rpi: 3, wsl: 2}`
- Large values: `{macos: 1000}`

**Event IDs**:
- Format: `event-{descriptor}-{number}`
- Examples: `event-test-001`, `event-agent-c-d-flow`

**Acknowledgment IDs**:
- Format: `ack-{event-id}-{node-id}-{sequence}`
- Examples: `ack-event-001-macos-1`, `ack-event-001-rpi-2`

---

## 6. Success Criteria

### Unit Tests
- All 21 tests pass
- Coverage: 100% of vector clock and ancestry logic
- Execution time: < 100ms

### Integration Tests
- All 8 tests pass
- No race conditions under rapid append (5+ concurrent writes)
- File integrity maintained throughout
- Execution time: < 500ms per test

### End-to-End Tests
- All 8 tests pass
- Proper file organization by date
- Correct metadata preservation
- Proper error handling
- Execution time: < 2 seconds total

### Overall Success
- 37 tests passing
- No flaky tests (100% consistent pass rate)
- Coverage of all code paths
- Clear error messages on failure

---

## 7. Risk Mitigation

### Known Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| **File corruption from concurrent writes** | Use append-only JSONL with atomic operations |
| **Vector clock overflow** | Test with large numbers (1000+) |
| **Date directory changes at midnight** | Test runs capture current date once, use consistently |
| **Metadata corruption** | Roundtrip serialization tests verify preservation |
| **Missing event files** | Test empty log handling explicitly |

---

## 8. Maintenance and Evolution

### Test Maintenance Plan

1. **Review Cadence**: Quarterly
2. **Update Trigger**: Any changes to vector clock algorithm
3. **Regression Prevention**: Run full suite before merge
4. **Performance Targets**: Unit < 100ms, Integration < 500ms each

### Future Enhancements

- **Performance Tests**: Measure 1000+ acknowledgment chains
- **Stress Tests**: Simulate 100 concurrent agents
- **Snapshot Tests**: Verify JSONL output format stability
- **Property-Based Tests**: QuickCheck-style property verification

---

## 9. Troubleshooting Guide

### Common Failures

**Vector Clock Comparison Fails**
```
Expected: Some(true)
Got: Some(false)
```
Check vector clock values match test expectation. Verify dominance rules in `compare_vector_clocks()`.

**JSONL Append Test Fails**
```
Error: File count mismatch
```
Verify `append_ack_to_jsonl()` creates parent directories. Check file permissions.

**E2E Test Timing Issues**
```
Error: Event file not found
```
Ensure `TEST_OUTPUT` directory is created before test runs. Check `setup()` function.

### Debug Flags

**Verbose Output**:
```bash
RUST_LOG=debug cargo test --test events_ack_unit_tests -- --nocapture
```

**Preserve Test Artifacts**:
```bash
bash tests/e2e/agent_c_to_d_test.sh --preserve
# Artifacts saved to /tmp/nabi-events-ack-e2e-test-{PID}/
```

**Inspect JSONL Files**:
```bash
# Pretty-print JSONL (one JSON per line)
jq '.' /tmp/nabi-events-ack-e2e-test-{PID}/events/2025-11-17/*.ack.jsonl

# Count entries
wc -l /tmp/nabi-events-ack-e2e-test-{PID}/events/2025-11-17/*.ack.jsonl
```

---

## 10. Glossary

| Term | Definition |
|------|-----------|
| **Vector Clock** | A map of node IDs to counters tracking causal ordering |
| **Dominance** | VC A dominates VC B if all nodes >= and at least one > |
| **Causal Ancestor** | An acknowledgment whose VC is causally prior to another's |
| **JSONL** | JSON Lines: one complete JSON object per line |
| **Atomic** | File operation that never leaves partial state |
| **Concurrent** | Events with no causal ordering relationship |

---

## Appendix A: Test Execution Checklist

- [ ] All dependencies installed (`cargo`, `bash`, `jq`)
- [ ] Test environment has write access to `/tmp`
- [ ] No other tests running simultaneously (single-threaded mode for integration)
- [ ] Run unit tests first (fastest feedback)
- [ ] Run integration tests (validates file I/O)
- [ ] Run E2E tests (validates user workflows)
- [ ] All 37 tests passing before merge
- [ ] Test artifacts cleaned up or preserved as needed

---

## Appendix B: Code Review Checklist

- [ ] Vector clock comparison logic is correct
- [ ] Ancestry computation handles all cases
- [ ] JSONL append is truly atomic
- [ ] Metadata serialization preserves types
- [ ] Error handling covers edge cases
- [ ] Test isolation prevents cross-contamination
- [ ] No hardcoded paths (use test helpers)
- [ ] Performance targets achievable

---

**End of Test Strategy Document**
