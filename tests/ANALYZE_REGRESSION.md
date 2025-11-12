# Analyze Repo Regression Test Suite

Comprehensive regression tests for `nabi analyze repo` functionality covering cache reuse, force rebuild, language separation, and multi-agent scenarios.

## Quick Start

```bash
# Run all analyze regression tests
cd ~/nabia/core/nabi-cli
./tests/analyze_regression.sh

# Run specific scenario
./tests/analyze_regression.sh --scenario cache_reuse

# Verbose output
./tests/analyze_regression.sh --verbose

# Clean test cache
./tests/analyze_regression.sh --clean
```

## Test Scenarios

### Scenario 1: Cache Reuse (Happy Path)
**Validates**: Cached indices are reused when appropriate

**Test Steps**:
1. First run creates cache
2. Second run reuses cache
3. Verifies "Loading Cached Index" message
4. Verifies no new index generation

**Expected**: Cache is reused, metadata displayed

### Scenario 2: Force Rebuild
**Validates**: `--force` flag rebuilds index even when cache exists

**Test Steps**:
1. Create cache and note timestamp
2. Run with `--force`
3. Verify "Force Rebuild" message
4. Verify new timestamp is more recent

**Expected**: Index is rebuilt, timestamp updated

### Scenario 3: Language Separation
**Validates**: Different language analyses don't overwrite each other

**Test Steps**:
1. Create Python cache (`repo-hash-python/`)
2. Create Rust cache (`repo-hash-rust/`)
3. Verify both directories exist
4. Verify each contains correct language metadata

**Expected**: Both caches exist, no overwriting

### Scenario 4: Graph Query Resolution
**Validates**: Graph queries find correct indices with language matching

**Test Steps**:
1. Create Python index
2. Query graph
3. Verify query finds correct index

**Expected**: Graph queries resolve to correct index directory

### Scenario 5: Multi-Agent Cache Sharing
**Validates**: Multiple agents can safely share cached indices

**Test Steps**:
1. Agent A creates cache
2. Agent B reuses cache
3. Both agents query index simultaneously
4. Verify no race conditions

**Expected**: Second agent reuses cache, both can query concurrently

### Scenario 6: Cache Directory Naming
**Validates**: Cache directory naming follows pattern `repo-hash-lang`

**Test Steps**:
1. Create cache
2. Verify directory name format
3. Verify hash is deterministic

**Expected**: Format `repo-hash-lang`, hash is deterministic

### Scenario 7: Missing Cache Handling
**Validates**: Graceful handling when cache doesn't exist

**Test Steps**:
1. Remove cache directory
2. Query graph
3. Verify error message suggests running analyze

**Expected**: Clear error message, suggests running analyze

### Scenario 8: Corrupted Cache Handling
**Validates**: Handling of corrupted cache files

**Test Steps**:
1. Create valid cache
2. Corrupt metadata.json
3. Run analyze
4. Verify rebuilds or shows error

**Expected**: Detects corruption, rebuilds or shows error

### Scenario 9: Concurrent Analysis
**Validates**: Multiple simultaneous analyses don't conflict

**Test Steps**:
1. Start two analyses concurrently
2. Wait for both to complete
3. Verify cache is valid

**Expected**: Both complete, cache remains valid

### Scenario 10: Top-Level vs Repo Subcommand
**Validates**: Both command forms work identically

**Test Steps**:
1. Run `nabi analyze repo <path>`
2. Run `nabi repo analyze <path>`
3. Verify both use same cache

**Expected**: Both commands produce same cache, behave identically

## Test Fixtures

The test suite uses a sample repository at `tests/fixtures/test-repo/` containing:
- Python code (`src/main.py`)
- Rust code (`src/lib.rs`)
- Language indicators (`Cargo.toml`, `pyproject.toml`)

## Integration

The analyze regression tests are integrated into the main regression suite:

```bash
# Run all regression tests (includes analyze tests)
./tests/regression.sh
```

## Success Criteria

All scenarios must pass:
- ✓ Cache reuse works correctly
- ✓ Force rebuild works correctly
- ✓ Language separation prevents overwriting
- ✓ Graph queries resolve correctly
- ✓ Multi-agent scenarios work safely
- ✓ Error handling is graceful
- ✓ No race conditions or corruption

## Troubleshooting

### Tests fail with "Test repository not found"
Ensure fixtures are created:
```bash
ls tests/fixtures/test-repo/
```

### Cache directory not found
Check XDG_CACHE_HOME or ~/.cache/nabi/codebase-graphs

### Hash extraction fails
This is a warning, not a failure. The hash calculation test may be inconclusive but doesn't affect functionality.

## Related Documentation

- [TESTING.md](../TESTING.md) - Main testing guide
- [regression.sh](./regression.sh) - Main regression test suite
- [analyze_regression.sh](./analyze_regression.sh) - This test suite
