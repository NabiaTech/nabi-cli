#!/bin/bash

###############################################################################
# Run All Events ACK Tests
#
# This script executes the complete test suite:
# - 21 unit tests
# - 8 integration tests
# - 8 end-to-end tests
#
# Usage: bash run_events_ack_tests.sh [OPTIONS]
# Options:
#   --unit         Run only unit tests
#   --integration  Run only integration tests
#   --e2e          Run only E2E tests
#   --preserve     Preserve test artifacts
#   --verbose      Show detailed output
#   --help         Show this help message
###############################################################################

set -euo pipefail

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Test configuration
RUN_UNIT=true
RUN_INTEGRATION=true
RUN_E2E=true
PRESERVE_ARTIFACTS=false
VERBOSE=false
START_TIME=$(date +%s)

###############################################################################
# Logging Functions
###############################################################################

log_section() {
    echo -e "\n${BOLD}${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BOLD}${BLUE}║${NC} $*"
    echo -e "${BOLD}${BLUE}╚════════════════════════════════════════════════════════╝${NC}\n"
}

log_subsection() {
    echo -e "\n${BOLD}${BLUE}── $*${NC}"
}

log_success() {
    echo -e "${GREEN}✓${NC} $*"
}

log_failure() {
    echo -e "${RED}✗${NC} $*"
}

log_info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $*"
}

###############################################################################
# Argument Parsing
###############################################################################

parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --unit)
                RUN_UNIT=true
                RUN_INTEGRATION=false
                RUN_E2E=false
                shift
                ;;
            --integration)
                RUN_UNIT=false
                RUN_INTEGRATION=true
                RUN_E2E=false
                shift
                ;;
            --e2e)
                RUN_UNIT=false
                RUN_INTEGRATION=false
                RUN_E2E=true
                shift
                ;;
            --preserve)
                PRESERVE_ARTIFACTS=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                log_failure "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

show_help() {
    cat <<EOF
${BOLD}Events ACK Test Suite Runner${NC}

${BOLD}Usage:${NC}
  bash run_events_ack_tests.sh [OPTIONS]

${BOLD}Options:${NC}
  --unit              Run only unit tests (21 tests)
  --integration       Run only integration tests (8 tests)
  --e2e               Run only E2E tests (8 tests)
  --preserve          Preserve test artifacts (E2E tests)
  --verbose           Show detailed test output
  --help              Show this help message

${BOLD}Examples:${NC}
  # Run all tests (default)
  bash run_events_ack_tests.sh

  # Run only unit tests
  bash run_events_ack_tests.sh --unit

  # Run integration and E2E tests
  bash run_events_ack_tests.sh --integration --e2e

  # Run E2E tests with artifacts preserved
  bash run_events_ack_tests.sh --e2e --preserve

${BOLD}Test Counts:${NC}
  Unit Tests:          21
  Integration Tests:    8
  End-to-End Tests:     8
  ━━━━━━━━━━━━━━━━━━━━━━
  Total:               37

${BOLD}Expected Execution Times:${NC}
  Unit tests:          ~50ms
  Integration tests:   ~400ms
  E2E tests:           ~2s
  Total:               ~2.5s

EOF
}

###############################################################################
# Test Execution Functions
###############################################################################

run_unit_tests() {
    log_subsection "Running Unit Tests (21 tests)"

    local test_dir="$PROJECT_ROOT/tests"

    if [ ! -f "$test_dir/events_ack_unit_tests.rs" ]; then
        log_failure "Unit test file not found: $test_dir/events_ack_unit_tests.rs"
        return 1
    fi

    cd "$PROJECT_ROOT"

    if [ "$VERBOSE" = true ]; then
        cargo test --test events_ack_unit_tests -- --nocapture --show-output
    else
        cargo test --test events_ack_unit_tests -- --nocapture
    fi

    log_success "Unit tests passed"
    return 0
}

run_integration_tests() {
    log_subsection "Running Integration Tests (8 tests)"

    local test_dir="$PROJECT_ROOT/tests"

    if [ ! -f "$test_dir/events_ack_integration_tests.rs" ]; then
        log_failure "Integration test file not found: $test_dir/events_ack_integration_tests.rs"
        return 1
    fi

    cd "$PROJECT_ROOT"

    if [ "$VERBOSE" = true ]; then
        cargo test --test events_ack_integration_tests -- --nocapture --show-output --test-threads=1
    else
        cargo test --test events_ack_integration_tests -- --nocapture --test-threads=1
    fi

    log_success "Integration tests passed"
    return 0
}

run_e2e_tests() {
    log_subsection "Running End-to-End Tests (8 tests)"

    local test_dir="$PROJECT_ROOT/tests"
    local e2e_script="$test_dir/e2e/agent_c_to_d_test.sh"

    if [ ! -f "$e2e_script" ]; then
        log_failure "E2E test script not found: $e2e_script"
        return 1
    fi

    if [ ! -x "$e2e_script" ]; then
        log_warning "E2E test script not executable, fixing..."
        chmod +x "$e2e_script"
    fi

    if [ "$PRESERVE_ARTIFACTS" = true ]; then
        bash "$e2e_script" --preserve
    else
        bash "$e2e_script"
    fi

    log_success "E2E tests passed"
    return 0
}

###############################################################################
# Summary Functions
###############################################################################

print_summary() {
    local unit_result="$1"
    local integration_result="$2"
    local e2e_result="$3"
    local elapsed_seconds="$4"

    local unit_status=$([ "$unit_result" -eq 0 ] && echo "PASSED" || echo "FAILED")
    local integration_status=$([ "$integration_result" -eq 0 ] && echo "PASSED" || echo "FAILED")
    local e2e_status=$([ "$e2e_result" -eq 0 ] && echo "PASSED" || echo "FAILED")

    local unit_color=$([ "$unit_result" -eq 0 ] && echo "$GREEN" || echo "$RED")
    local integration_color=$([ "$integration_result" -eq 0 ] && echo "$GREEN" || echo "$RED")
    local e2e_color=$([ "$e2e_result" -eq 0 ] && echo "$GREEN" || echo "$RED")

    log_section "Test Results Summary"

    if [ "$RUN_UNIT" = true ]; then
        echo -e "${unit_color}━${NC} Unit Tests (21):             ${unit_color}${unit_status}${NC}"
    fi

    if [ "$RUN_INTEGRATION" = true ]; then
        echo -e "${integration_color}━${NC} Integration Tests (8):        ${integration_color}${integration_status}${NC}"
    fi

    if [ "$RUN_E2E" = true ]; then
        echo -e "${e2e_color}━${NC} End-to-End Tests (8):         ${e2e_color}${e2e_status}${NC}"
    fi

    echo ""

    local total_tests=0
    [ "$RUN_UNIT" = true ] && total_tests=$((total_tests + 21))
    [ "$RUN_INTEGRATION" = true ] && total_tests=$((total_tests + 8))
    [ "$RUN_E2E" = true ] && total_tests=$((total_tests + 8))

    # Format elapsed time
    local minutes=$((elapsed_seconds / 60))
    local seconds=$((elapsed_seconds % 60))
    local time_str="$((minutes))m $((seconds))s"

    if [ "$unit_result" -eq 0 ] && [ "$integration_result" -eq 0 ] && [ "$e2e_result" -eq 0 ]; then
        log_success "All $total_tests tests passed in $time_str"
        return 0
    else
        log_failure "Some tests failed"
        return 1
    fi
}

###############################################################################
# Main Execution
###############################################################################

main() {
    parse_arguments "$@"

    log_section "Events ACK Test Suite Runner"

    log_info "Test Configuration:"
    echo "  Unit Tests:        $([[ "$RUN_UNIT" == "true" ]] && echo "✓" || echo "✗")"
    echo "  Integration Tests: $([[ "$RUN_INTEGRATION" == "true" ]] && echo "✓" || echo "✗")"
    echo "  E2E Tests:         $([[ "$RUN_E2E" == "true" ]] && echo "✓" || echo "✗")"
    echo "  Preserve Artifacts: $([[ "$PRESERVE_ARTIFACTS" == "true" ]] && echo "✓" || echo "✗")"
    echo "  Verbose Output:    $([[ "$VERBOSE" == "true" ]] && echo "✓" || echo "✗")"
    echo ""

    # Run selected test suites
    local unit_result=0
    local integration_result=0
    local e2e_result=0

    if [ "$RUN_UNIT" = true ]; then
        run_unit_tests || unit_result=$?
    fi

    if [ "$RUN_INTEGRATION" = true ]; then
        run_integration_tests || integration_result=$?
    fi

    if [ "$RUN_E2E" = true ]; then
        run_e2e_tests || e2e_result=$?
    fi

    # Calculate elapsed time
    local end_time=$(date +%s)
    local elapsed=$((end_time - START_TIME))

    # Print summary
    print_summary "$unit_result" "$integration_result" "$e2e_result" "$elapsed"
    local summary_result=$?

    echo ""

    if [ "$summary_result" -eq 0 ]; then
        echo -e "${GREEN}${BOLD}🎉 All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}${BOLD}❌ Some tests failed${NC}"
        exit 1
    fi
}

# Execute main function
main "$@"
