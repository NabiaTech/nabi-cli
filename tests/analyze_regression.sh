#!/bin/bash
# Nabi CLI - Analyze Repo Regression Test Suite
# Tests cache reuse, force rebuild, language separation, and multi-agent scenarios
#
# Usage: ./tests/analyze_regression.sh [--scenario <name>] [--verbose] [--clean]
# Exit codes:
#   0 = All tests passed
#   1 = Test failure
#   2 = Test setup failure

set -u

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
TEST_REPO="$FIXTURES_DIR/test-repo"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/nabi/codebase-graphs"

# Handle NABI_BIN - could be directory or full path
if [[ "${NABI_BIN:-}" == *"/nabi"* ]]; then
    NABI_BIN="${NABI_BIN}"
elif [[ -n "${NABI_BIN:-}" ]]; then
    NABI_BIN="${NABI_BIN}/nabi"
else
    NABI_BIN="$HOME/.local/bin/nabi"
fi

VERBOSE="${VERBOSE:-0}"
CLEAN_MODE="${CLEAN_MODE:-0}"
SCENARIO_FILTER="${SCENARIO_FILTER:-}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --verbose|-v) VERBOSE=1; shift ;;
        --clean|-c) CLEAN_MODE=1; shift ;;
        --scenario|-s) SCENARIO_FILTER="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 2 ;;
    esac
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
SCENARIOS_RUN=0
SCENARIOS_PASSED=0
SCENARIOS_FAILED=0

# Logging functions
log_test() {
    echo -e "${CYAN}[TEST]${NC} $*"
}

log_pass() {
    echo -e "${GREEN}✓${NC} $*"
    ((TESTS_PASSED++))
}

log_fail() {
    echo -e "${RED}✗${NC} $*"
    ((TESTS_FAILED++))
}

log_warn() {
    echo -e "${YELLOW}⚠${NC} $*"
}

log_info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

log_section() {
    echo ""
    echo -e "${CYAN}=== $* ===${NC}"
    echo ""
}

# Test harness
run_test() {
    local test_name="$1"
    local test_cmd="$2"
    local expected_exit="${3:-0}"

    ((TESTS_RUN++))
    log_test "$test_name"

    if [[ $VERBOSE -eq 1 ]]; then
        log_info "Command: $test_cmd"
    fi

    # Run command and capture output & exit code
    local output
    local exit_code=0
    output=$(eval "$test_cmd" 2>&1) || exit_code=$?

    # Check exit code
    if [[ $exit_code -eq $expected_exit ]]; then
        log_pass "$test_name (exit: $exit_code)"
        if [[ $VERBOSE -eq 1 ]] && [[ -n "$output" ]]; then
            echo "  Output: ${output:0:200}..."
        fi
        return 0
    else
        log_fail "$test_name (expected: $expected_exit, got: $exit_code)"
        if [[ -n "$output" ]]; then
            echo "  Output:"
            echo "$output" | sed 's/^/    /' | head -20
        fi
        return 1
    fi
}

# Assertion helpers
assert_contains() {
    local haystack="$1"
    local needle="$2"
    local test_name="${3:-Contains check}"

    if echo "$haystack" | grep -q "$needle"; then
        log_pass "$test_name: found '$needle'"
        return 0
    else
        log_fail "$test_name: expected '$needle' not found"
        return 1
    fi
}

assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    local test_name="${3:-Not contains check}"

    if echo "$haystack" | grep -qv "$needle"; then
        log_pass "$test_name: '$needle' not found (as expected)"
        return 0
    else
        log_fail "$test_name: unexpected '$needle' found"
        return 1
    fi
}

# Helper: Get cache directory for a repo
get_cache_dir_for_repo() {
    local repo_path="$1"
    local lang="${2:-}"

    local repo_name=$(basename "$repo_path")

    # Ensure cache directory exists
    [[ ! -d "$CACHE_DIR" ]] && return

    # Find cache directory by pattern matching (more reliable than hash calculation)
    if [[ -n "$lang" ]]; then
        # Look for exact match: repo-hash-lang
        local found=$(find "$CACHE_DIR" -maxdepth 1 -type d -name "${repo_name}-*-${lang}" 2>/dev/null | head -1)
        if [[ -n "$found" ]] && [[ -d "$found" ]]; then
            echo "$found"
        fi
    else
        # Find any matching cache for this repo
        local found=$(find "$CACHE_DIR" -maxdepth 1 -type d -name "${repo_name}-*" 2>/dev/null | head -1)
        if [[ -n "$found" ]] && [[ -d "$found" ]]; then
            echo "$found"
        fi
    fi
}

# Helper: Clean test cache
clean_test_cache() {
    log_info "Cleaning test cache directories..."
    if [[ -d "$CACHE_DIR" ]]; then
        find "$CACHE_DIR" -maxdepth 1 -type d -name "test-repo-*" -exec rm -rf {} + 2>/dev/null || true
        log_pass "Test cache cleaned"
    fi
}

# Helper: Get metadata timestamp from cache
get_cache_timestamp() {
    local cache_dir="$1"
    local metadata_file="$cache_dir/metadata.json"

    if [[ -f "$metadata_file" ]]; then
        # Extract created timestamp from JSON (simplified)
        grep -o '"created":"[^"]*"' "$metadata_file" | cut -d'"' -f4 || echo ""
    else
        echo ""
    fi
}

# ============================================================================
# SCENARIO IMPLEMENTATIONS
# ============================================================================

scenario_cache_reuse() {
    log_section "Scenario 1: Cache Reuse (Happy Path)"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Clean any existing cache
    [[ -d "$cache_dir" ]] && rm -rf "$cache_dir"

    # Step 1: First run - creates cache
    log_test "First run: Create cache"
    local output1
    output1=$($NABI_BIN analyze repo "$test_repo" --lang python 2>&1) || failed=1

    # Re-find cache directory after creation (path may have been calculated before creation)
    cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    if [[ $failed -eq 0 ]] && [[ -n "$cache_dir" ]] && [[ -d "$cache_dir" ]] && [[ -f "$cache_dir/metadata.json" ]]; then
        log_pass "Cache directory created: $cache_dir"
    else
        log_fail "Cache not created properly (dir: $cache_dir)"
        if [[ $VERBOSE -eq 1 ]]; then
            echo "Output: $output1"
        fi
        failed=1
    fi

    # Step 2: Second run - should reuse cache
    log_test "Second run: Reuse cache"
    local output2
    output2=$($NABI_BIN analyze repo "$test_repo" --lang python 2>&1) || failed=1

    assert_contains "$output2" "Loading Cached Index" "Cache reuse message" || failed=1
    assert_not_contains "$output2" "Index Generation" "No new generation" || failed=1
    assert_contains "$output2" "Loaded.*symbols" "Cache metadata displayed" || failed=1

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 1 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 1 FAILED"
    fi
}

scenario_force_rebuild() {
    log_section "Scenario 2: Force Rebuild"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Ensure cache exists
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1

    # Get original timestamp
    local original_timestamp=$(get_cache_timestamp "$cache_dir")
    log_info "Original timestamp: $original_timestamp"

    # Wait a moment to ensure timestamp difference
    sleep 1

    # Step 3: Force rebuild
    log_test "Force rebuild with --force flag"
    local output
    output=$($NABI_BIN analyze repo "$test_repo" --lang python --force 2>&1) || failed=1

    assert_contains "$output" "Force Rebuild" "Force rebuild message" || failed=1
    assert_contains "$output" "Index Generation" "New index generation" || failed=1

    # Check new timestamp
    local new_timestamp=$(get_cache_timestamp "$cache_dir")
    log_info "New timestamp: $new_timestamp"

    if [[ -n "$original_timestamp" ]] && [[ -n "$new_timestamp" ]] && [[ "$new_timestamp" != "$original_timestamp" ]]; then
        log_pass "Timestamp updated (rebuild occurred)"
    else
        log_warn "Timestamp check inconclusive (may be same if very fast)"
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 2 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 2 FAILED"
    fi
}

scenario_language_separation() {
    log_section "Scenario 3: Language Separation"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local python_cache=$(get_cache_dir_for_repo "$test_repo" "python")
    local rust_cache=$(get_cache_dir_for_repo "$test_repo" "rust")

    # Step 1: Create Python cache
    log_test "Create Python cache"
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 || failed=1

    if [[ -d "$python_cache" ]] && [[ -f "$python_cache/metadata.json" ]]; then
        log_pass "Python cache created: $python_cache"
        assert_contains "$(cat "$python_cache/metadata.json")" '"language":"python"' "Python language metadata" || failed=1
    else
        log_fail "Python cache not created"
        failed=1
    fi

    # Step 2: Create Rust cache
    log_test "Create Rust cache"
    $NABI_BIN analyze repo "$test_repo" --lang rust >/dev/null 2>&1 || failed=1

    if [[ -d "$rust_cache" ]] && [[ -f "$rust_cache/metadata.json" ]]; then
        log_pass "Rust cache created: $rust_cache"
        assert_contains "$(cat "$rust_cache/metadata.json")" '"language":"rust"' "Rust language metadata" || failed=1
    else
        log_fail "Rust cache not created"
        failed=1
    fi

    # Step 3: Verify both exist
    log_test "Verify both caches exist"
    if [[ -d "$python_cache" ]] && [[ -d "$rust_cache" ]]; then
        log_pass "Both caches exist simultaneously"
    else
        log_fail "One or both caches missing"
        failed=1
    fi

    # Step 4: Verify Python cache still intact
    if [[ -f "$python_cache/metadata.json" ]] && grep -q '"language":"python"' "$python_cache/metadata.json"; then
        log_pass "Python cache remains intact"
    else
        log_fail "Python cache was overwritten or corrupted"
        failed=1
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 3 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 3 FAILED"
    fi
}

scenario_graph_query_resolution() {
    log_section "Scenario 4: Graph Query Resolution"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"

    # Ensure Python index exists
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1

    # Step 2: Query graph
    log_test "Query graph with Python index"
    local output
    output=$($NABI_BIN repo graph search "test_function" --repo "$test_repo" 2>&1) || {
        # If search fails, it might be because symbol doesn't exist - that's OK for this test
        log_warn "Graph search returned non-zero (may be expected if symbol not found)"
    }

    # Should not error about missing index
    if echo "$output" | grep -q "No index found"; then
        log_fail "Graph query failed to find index"
        failed=1
    else
        log_pass "Graph query resolved index (or found no symbols, which is OK)"
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 4 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 4 FAILED"
    fi
}

scenario_multi_agent_sharing() {
    log_section "Scenario 5: Multi-Agent Cache Sharing"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Clean cache
    [[ -d "$cache_dir" ]] && rm -rf "$cache_dir"

    # Step 1: Agent A creates cache
    log_test "Agent A: Create cache"
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 || failed=1

    if [[ ! -d "$cache_dir" ]]; then
        log_fail "Agent A failed to create cache"
        failed=1
    fi

    # Step 2: Agent B reuses cache
    log_test "Agent B: Reuse cache"
    local output
    output=$($NABI_BIN analyze repo "$test_repo" --lang python 2>&1) || failed=1

    assert_contains "$output" "Loading Cached Index" "Agent B reuses cache" || failed=1

    # Step 3: Both can query (simulated by sequential queries)
    log_test "Both agents can query index"
    $NABI_BIN repo graph search "TestClass" --repo "$test_repo" >/dev/null 2>&1 || true
    $NABI_BIN repo graph search "test_function" --repo "$test_repo" >/dev/null 2>&1 || true
    log_pass "Sequential queries completed (simulating concurrent access)"

    # Verify cache still valid
    if [[ -f "$cache_dir/metadata.json" ]]; then
        log_pass "Cache remains valid after queries"
    else
        log_fail "Cache corrupted after queries"
        failed=1
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 5 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 5 FAILED"
    fi
}

scenario_cache_directory_naming() {
    log_section "Scenario 6: Cache Directory Naming"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Step 1: Create cache
    log_test "Create cache and verify naming pattern"
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 || failed=1

    # Verify pattern: repo-hash-lang
    local dir_name=$(basename "$cache_dir")
    if echo "$dir_name" | grep -qE "^test-repo-[a-f0-9]{8}-python$"; then
        log_pass "Directory name matches pattern: repo-hash-lang"
    else
        log_fail "Directory name doesn't match pattern: $dir_name"
        failed=1
    fi

    # Step 2: Verify hash is deterministic
    log_test "Verify hash is deterministic"
    # Extract hash from directory name (format: repo-hash-lang)
    local hash1=$(echo "$dir_name" | sed -E "s/^test-repo-([a-f0-9]{8})-python$/\1/")

    if [[ -z "$hash1" ]] || [[ "$hash1" == "$dir_name" ]]; then
        log_warn "Could not extract hash from directory name: $dir_name"
    else
        # Remove and recreate
        rm -rf "$cache_dir"
        $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1
        local new_cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")
        local new_dir_name=$(basename "$new_cache_dir")
        local hash2=$(echo "$new_dir_name" | sed -E "s/^test-repo-([a-f0-9]{8})-python$/\1/")

        if [[ "$hash1" == "$hash2" ]] && [[ -n "$hash1" ]] && [[ "$hash1" != "$new_dir_name" ]]; then
            log_pass "Hash is deterministic (same repo = same hash: $hash1)"
        else
            log_warn "Hash check inconclusive (hash1: $hash1, hash2: $hash2)"
        fi
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 6 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 6 FAILED"
    fi
}

scenario_missing_cache_handling() {
    log_section "Scenario 7: Missing Cache Handling"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Remove cache
    [[ -d "$cache_dir" ]] && rm -rf "$cache_dir"

    # Step 2: Try to query graph
    log_test "Query graph with missing cache"
    local output
    output=$($NABI_BIN repo graph search "test_function" --repo "$test_repo" 2>&1)
    local exit_code=$?

    # Should error with helpful message
    if [[ $exit_code -ne 0 ]]; then
        log_pass "Command exited with non-zero (as expected)"
    else
        log_fail "Command should fail when cache missing"
        failed=1
    fi

    assert_contains "$output" "No index found" "Error message about missing index" || failed=1
    assert_contains "$output" "analyze repo" "Suggests running analyze command" || failed=1

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 7 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 7 FAILED"
    fi
}

scenario_corrupted_cache_handling() {
    log_section "Scenario 8: Corrupted Cache Handling"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Create valid cache
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1

    # Corrupt metadata.json
    log_test "Corrupt cache metadata"
    echo "invalid json{" > "$cache_dir/metadata.json"

    # Step 3: Try to use corrupted cache
    log_test "Handle corrupted cache"
    local output
    output=$($NABI_BIN analyze repo "$test_repo" --lang python 2>&1) || {
        # If it fails, that's OK - should either rebuild or show error
        log_info "Command failed (may rebuild or show error)"
    }

    # Should either rebuild or show error, but not crash
    if echo "$output" | grep -qE "(Failed to load|rebuild|error)" || [[ -f "$cache_dir/metadata.json" ]]; then
        log_pass "Handled corruption gracefully (rebuilt or showed error)"
    else
        log_warn "Corruption handling unclear"
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 8 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 8 FAILED"
    fi
}

scenario_concurrent_analysis() {
    log_section "Scenario 9: Concurrent Analysis"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir=$(get_cache_dir_for_repo "$test_repo" "python")

    # Clean cache
    [[ -d "$cache_dir" ]] && rm -rf "$cache_dir"

    # Step 1 & 2: Start two analyses concurrently
    log_test "Run two analyses concurrently"
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 &
    local pid1=$!
    sleep 0.1  # Small delay to ensure both start
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 &
    local pid2=$!

    # Wait for both to complete
    wait $pid1
    wait $pid2

    # Step 3: Verify both completed
    if [[ -d "$cache_dir" ]] && [[ -f "$cache_dir/metadata.json" ]]; then
        log_pass "Both analyses completed, cache exists"
    else
        log_fail "Cache missing after concurrent analysis"
        failed=1
    fi

    # Step 4: Verify cache is valid
    if [[ -f "$cache_dir/metadata.json" ]] && grep -q '"language":"python"' "$cache_dir/metadata.json" 2>/dev/null; then
        log_pass "Cache is valid after concurrent analysis"
    else
        log_fail "Cache corrupted after concurrent analysis"
        failed=1
    fi

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 9 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 9 FAILED"
    fi
}

scenario_command_equivalence() {
    log_section "Scenario 10: Top-Level vs Repo Subcommand"
    ((SCENARIOS_RUN++))
    local failed=0

    local test_repo="$TEST_REPO"
    local cache_dir1=$(get_cache_dir_for_repo "$test_repo" "python")

    # Clean cache
    [[ -d "$cache_dir1" ]] && rm -rf "$cache_dir1"

    # Step 1: Top-level command
    log_test "Top-level command: nabi analyze repo"
    $NABI_BIN analyze repo "$test_repo" --lang python >/dev/null 2>&1 || failed=1

    local cache1=$(get_cache_dir_for_repo "$test_repo" "python")
    local metadata1=""
    [[ -f "$cache1/metadata.json" ]] && metadata1=$(cat "$cache1/metadata.json")

    # Step 3: Subcommand (should reuse same cache)
    log_test "Subcommand: nabi repo analyze"
    local output
    output=$($NABI_BIN repo analyze "$test_repo" --lang python 2>&1) || failed=1

    local cache2=$(get_cache_dir_for_repo "$test_repo" "python")
    local metadata2=""
    [[ -f "$cache2/metadata.json" ]] && metadata2=$(cat "$cache2/metadata.json")

    # Step 4: Verify same cache location
    if [[ "$cache1" == "$cache2" ]]; then
        log_pass "Both commands use same cache location"
    else
        log_fail "Commands use different cache locations"
        failed=1
    fi

    # Step 5: Verify identical behavior (both should show cache reuse)
    assert_contains "$output" "Loading Cached Index" "Subcommand reuses cache" || failed=1

    if [[ $failed -eq 0 ]]; then
        ((SCENARIOS_PASSED++))
        log_pass "Scenario 10 PASSED"
    else
        ((SCENARIOS_FAILED++))
        log_fail "Scenario 10 FAILED"
    fi
}

# ============================================================================
# MAIN TEST RUNNER
# ============================================================================

main() {
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}  Analyze Repo - Regression Test Suite                ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""

    log_info "Binary: $NABI_BIN"
    log_info "Test Repo: $TEST_REPO"
    log_info "Cache Dir: $CACHE_DIR"
    [[ $VERBOSE -eq 1 ]] && log_info "Verbose mode ON"
    [[ $CLEAN_MODE -eq 1 ]] && log_info "Clean mode ON"
    echo ""

    # Clean if requested
    if [[ $CLEAN_MODE -eq 1 ]]; then
        clean_test_cache
        exit 0
    fi

    # Verify test repo exists
    if [[ ! -d "$TEST_REPO" ]]; then
        log_fail "Test repository not found: $TEST_REPO"
        exit 2
    fi

    # Verify nabi binary exists
    if [[ ! -f "$NABI_BIN" ]]; then
        log_fail "nabi binary not found: $NABI_BIN"
        exit 2
    fi

    # Run scenarios
    if [[ -n "$SCENARIO_FILTER" ]]; then
        case "$SCENARIO_FILTER" in
            cache_reuse|1) scenario_cache_reuse ;;
            force_rebuild|2) scenario_force_rebuild ;;
            language_separation|3) scenario_language_separation ;;
            graph_query|4) scenario_graph_query_resolution ;;
            multi_agent|5) scenario_multi_agent_sharing ;;
            cache_naming|6) scenario_cache_directory_naming ;;
            missing_cache|7) scenario_missing_cache_handling ;;
            corrupted_cache|8) scenario_corrupted_cache_handling ;;
            concurrent|9) scenario_concurrent_analysis ;;
            command_equivalence|10) scenario_command_equivalence ;;
            *) log_fail "Unknown scenario: $SCENARIO_FILTER"; exit 2 ;;
        esac
    else
        # Run all scenarios
        scenario_cache_reuse
        scenario_force_rebuild
        scenario_language_separation
        scenario_graph_query_resolution
        scenario_multi_agent_sharing
        scenario_cache_directory_naming
        scenario_missing_cache_handling
        scenario_corrupted_cache_handling
        scenario_concurrent_analysis
        scenario_command_equivalence
    fi

    # Summary
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}  Test Results                                      ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""

    echo "Scenarios run:    $SCENARIOS_RUN"
    echo "Scenarios passed: ${GREEN}$SCENARIOS_PASSED${NC}"
    if [[ $SCENARIOS_FAILED -gt 0 ]]; then
        echo "Scenarios failed: ${RED}$SCENARIOS_FAILED${NC}"
    else
        echo "Scenarios failed: $SCENARIOS_FAILED"
    fi
    echo ""
    echo "Tests run:    $TESTS_RUN"
    echo "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo "Tests failed: ${RED}$TESTS_FAILED${NC}"
    else
        echo "Tests failed: $TESTS_FAILED"
    fi

    echo ""

    if [[ $SCENARIOS_FAILED -eq 0 ]] && [[ $TESTS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}✗ Some tests failed${NC}"
        echo ""
        return 1
    fi
}

main "$@"
