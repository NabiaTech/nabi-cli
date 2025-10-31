#!/bin/bash
# Nabi CLI - Regression Test Harness
# Ensures critical functionality remains stable across refactoring
# Run after any Rust CLI changes or commander modifications
#
# Usage: ./tests/regression.sh [--verbose] [--fix]
# Exit codes:
#   0 = All tests passed
#   1 = Test failure
#   2 = Test setup failure

set -u
# Note: Not using -e so tests can continue even if individual commands fail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Handle NABI_BIN - could be directory or full path
if [[ "${NABI_BIN:-}" == *"/nabi"* ]]; then
    # Already full path to nabi binary
    :
elif [[ -n "${NABI_BIN:-}" ]]; then
    # Directory only, append binary name
    NABI_BIN="${NABI_BIN}/nabi"
else
    # Not set, use default
    NABI_BIN="$HOME/.local/bin/nabi"
fi

VERBOSE="${VERBOSE:-0}"
FIX_MODE="${FIX_MODE:-0}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --verbose|-v) VERBOSE=1; shift ;;
        --fix|-f) FIX_MODE=1; shift ;;
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
            echo "  Output: ${output:0:100}..."
        fi
        return 0
    else
        log_fail "$test_name (expected: $expected_exit, got: $exit_code)"
        if [[ -n "$output" ]]; then
            echo "  Output:"
            echo "$output" | sed 's/^/    /'
        fi
        return 1
    fi
}

# Assertion helpers
assert_contains() {
    local haystack="$1"
    local needle="$2"

    if echo "$haystack" | grep -q "$needle"; then
        return 0
    else
        return 1
    fi
}

# ============================================================================
# TEST SUITES
# ============================================================================

test_binary_exists() {
    log_section "Binary Availability"

    if [[ ! -f "$NABI_BIN" ]]; then
        log_fail "nabi binary not found at $NABI_BIN"
        log_info "Build with: cd $PROJECT_ROOT && cargo build --release"
        return 1
    fi

    log_pass "nabi binary exists at $NABI_BIN"

    if [[ ! -x "$NABI_BIN" ]]; then
        log_fail "nabi binary is not executable"
        if [[ $FIX_MODE -eq 1 ]]; then
            chmod +x "$NABI_BIN"
            log_pass "Fixed: made nabi executable"
        fi
        return 1
    fi

    log_pass "nabi binary is executable"
}

test_help_commands() {
    log_section "Help & Version"

    run_test "nabi --help" "$NABI_BIN --help" 0 || true
    run_test "nabi --version" "$NABI_BIN --version" 0 || true
    run_test "nabi help" "$NABI_BIN help" 0 || true
}

test_routing() {
    log_section "Command Routing (Critical Path)"

    # These should route but not execute (commanders are stubs)
    run_test "nabi claude --help" "$NABI_BIN claude --help" 0 || true
    run_test "nabi data --help" "$NABI_BIN data --help" 0 || true
    run_test "nabi federation --help" "$NABI_BIN federation --help" 0 || true
    run_test "nabi self --help" "$NABI_BIN self --help" 0 || true
}

test_commander_routing() {
    log_section "Commander Routing (Integration Tests)"

    # Test that commands route to the right places
    # These will fail or show placeholders, but shouldn't crash the router

    # Data commander - should route to Python CLI or show placeholder
    run_test "nabi data jsonl validate (routing)" \
        "$NABI_BIN data jsonl validate /tmp/test.jsonl 2>&1 | head -1" 0 || true

    # Claude commander - should route to Python CLI
    run_test "nabi claude session list (routing)" \
        "$NABI_BIN claude session list 2>&1 | head -1" 0 || true

    # Federation commander - should route correctly
    run_test "nabi federation agents (routing)" \
        "$NABI_BIN federation agents 2>&1 | head -1" 0 || true
}

test_health_checks() {
    log_section "Health Checks (nabi self doctor)"

    run_test "nabi self doctor" "$NABI_BIN self doctor" 0 || true
    run_test "nabi self config" "$NABI_BIN self config" 0 || true
}

test_critical_features() {
    log_section "Critical Features"

    # Test data commander with Python CLI fallback
    log_test "Data commander routes to Python CLI"
    local output
    output=$($NABI_BIN data jsonl validate /tmp/test.jsonl 2>&1 || true)

    # Should either:
    # 1. Route to native commander (if it exists)
    # 2. Route to Python CLI
    # 3. Show placeholder but not crash
    if echo "$output" | grep -qE "(Route|JSONL|Python|not yet)"; then
        log_pass "Data command routes without crashing"
    else
        log_fail "Data command output unexpected: $output"
    fi
}

test_commanders_directory() {
    log_section "Commander Infrastructure"

    local commanders_dir="$HOME/.config/nabi/commanders"

    if [[ ! -d "$commanders_dir" ]]; then
        log_fail "Commanders directory not found: $commanders_dir"
        return 1
    fi

    log_pass "Commanders directory exists"

    # Check for critical commanders
    local critical_commanders=("data" "claude" "federation")
    for commander in "${critical_commanders[@]}"; do
        local cmd_dir="$commanders_dir/$commander"
        if [[ -d "$cmd_dir" ]]; then
            log_pass "Commander '$commander' directory exists"

            # Check for executable
            if [[ -x "$cmd_dir/$commander" ]] || [[ -x "$cmd_dir/index.ts" ]] || [[ -f "$cmd_dir/$commander" ]]; then
                log_pass "  → Executable found"
            else
                log_warn "  → No executable found (expected: $cmd_dir/$commander)"
            fi
        else
            log_fail "Commander '$commander' directory missing: $cmd_dir"
        fi
    done
}

test_python_cli_fallback() {
    log_section "Python CLI Fallback System"

    local python_cli="$HOME/nabia/tools/nabi-python"

    if [[ ! -f "$python_cli" ]]; then
        log_fail "Python CLI not found at: $python_cli"
        log_info "Expected: ~/nabia/tools/nabi-python"
        return 1
    fi

    log_pass "Python CLI exists"

    if [[ ! -x "$python_cli" ]]; then
        log_fail "Python CLI is not executable"
        if [[ $FIX_MODE -eq 1 ]]; then
            chmod +x "$python_cli"
            log_pass "Fixed: made Python CLI executable"
        fi
        return 1
    fi

    log_pass "Python CLI is executable"
}

test_no_regression() {
    log_section "Regression Detection"

    # Test that we detect the premature data commander issue is fixed
    log_test "Data commander doesn't just error out immediately"

    local output
    output=$($NABI_BIN data jsonl validate /tmp/test.jsonl 2>&1 || true)

    # The issue was that it would error WITHOUT routing
    # Now it should either route or show a coherent error
    if echo "$output" | grep -q "Route"; then
        log_pass "Data command shows routing (issue fixed)"
    elif echo "$output" | grep -qE "(not (yet )?implemented|pending)"; then
        log_pass "Data command shows placeholder (acceptable fallback)"
    else
        log_warn "Data command output unclear: $output"
    fi
}

# ============================================================================
# MAIN TEST RUNNER
# ============================================================================

main() {
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}  Nabi CLI - Regression Test Suite                   ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""

    log_info "Binary: $NABI_BIN"
    log_info "Project: $PROJECT_ROOT"
    [[ $VERBOSE -eq 1 ]] && log_info "Verbose mode ON"
    [[ $FIX_MODE -eq 1 ]] && log_info "Fix mode ON"
    echo ""

    # Run all test suites
    test_binary_exists
    test_help_commands
    test_routing
    test_commander_routing
    test_health_checks
    test_critical_features
    test_commanders_directory
    test_python_cli_fallback
    test_no_regression

    # Summary
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}  Test Results                                      ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""

    echo "Tests run:    $TESTS_RUN"
    echo "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo "Tests failed: ${RED}$TESTS_FAILED${NC}"
    else
        echo "Tests failed: $TESTS_FAILED"
    fi

    echo ""

    if [[ $TESTS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}✗ $TESTS_FAILED test(s) failed${NC}"
        echo ""
        if [[ $FIX_MODE -eq 0 ]]; then
            log_info "Run with --fix to attempt automatic fixes"
        fi
        return 1
    fi
}

main "$@"
