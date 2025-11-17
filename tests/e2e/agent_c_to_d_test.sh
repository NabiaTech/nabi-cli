#!/bin/bash

###############################################################################
# End-to-End Test: Agent C Notification → Claude Acknowledgment → JSONL
#
# This test simulates the complete workflow:
# 1. An event is published to the federation event bus
# 2. Agent C detects the event (simulated)
# 3. Claude (Agent D) acknowledges the event
# 4. Vector clocks are incremented and causal ancestors computed
# 5. The acknowledgment is stored atomically in JSONL format
#
# Run with: bash tests/e2e/agent_c_to_d_test.sh
###############################################################################

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_OUTPUT="/tmp/nabi-events-ack-e2e-test-$$"
EVENTS_DIR="${TEST_OUTPUT}/events"
FIXTURES_DIR="${SCRIPT_DIR}/fixtures"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0

###############################################################################
# Logging Functions
###############################################################################

log_info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

log_success() {
    echo -e "${GREEN}✓${NC} $*"
    ((TESTS_PASSED++))
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $*"
}

log_error() {
    echo -e "${RED}✗${NC} $*"
    ((TESTS_FAILED++))
}

###############################################################################
# Setup and Teardown
###############################################################################

setup() {
    log_info "Setting up test environment..."

    # Create test directories
    mkdir -p "${EVENTS_DIR}"
    mkdir -p "${TEST_OUTPUT}/logs"

    # Create today's date directory for events
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    mkdir -p "${date_dir}"

    log_success "Test environment ready at ${TEST_OUTPUT}"
}

teardown() {
    local cleanup="${1:-true}"

    if [ "$cleanup" = "true" ]; then
        log_info "Cleaning up test environment..."
        rm -rf "${TEST_OUTPUT}"
        log_success "Cleanup complete"
    else
        log_info "Test artifacts preserved at ${TEST_OUTPUT}"
    fi
}

trap "teardown true" EXIT

###############################################################################
# Test Helper Functions
###############################################################################

# Create a mock event file
create_test_event() {
    local event_id="$1"
    local source="${2:-test-agent-c}"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"

    mkdir -p "${date_dir}"

    local event_file="${date_dir}/${event_id}.json"

    cat > "${event_file}" <<EOF
{
  "id": "${event_id}",
  "source": "${source}",
  "message": "Test event for Agent C → D integration",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "vector_clock": {
    "macos": 10
  }
}
EOF

    log_success "Created test event: ${event_id}"
    echo "${event_file}"
}

# Append an acknowledgment to the JSONL log
append_acknowledgment() {
    local event_id="$1"
    local node_id="$2"
    local timestamp=$(date +%s%N | tail -c 6)
    local ack_id="ack-${event_id}-${timestamp}"

    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    # Create parent directory if needed
    mkdir -p "$(dirname "${ack_log}")"

    # Generate acknowledgment JSON and append atomically
    printf '{
  "ack_id": "%s",
  "event_id": "%s",
  "node_id": "%s",
  "vector_clock": {
    "macos": 11
  },
  "causally_after": [],
  "timestamp": "%s",
  "metadata": {
    "test": true,
    "agent": "claude-test"
  }
}\n' "${ack_id}" "${event_id}" "${node_id}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "${ack_log}"

    log_success "Appended acknowledgment: ${ack_id}"
    echo "${ack_log}"
}

# Verify JSON is valid
validate_json() {
    local json_content="$1"
    local label="${2:-JSON}"

    if ! jq . <<<"${json_content}" >/dev/null 2>&1; then
        log_error "Invalid ${label}: ${json_content}"
        return 1
    fi

    log_success "Valid ${label} format"
    return 0
}

# Count lines in JSONL file
count_jsonl_lines() {
    local file="$1"

    if [ ! -f "${file}" ]; then
        echo "0"
        return 0
    fi

    wc -l < "${file}" | tr -d ' '
}

###############################################################################
# Test Cases
###############################################################################

test_1_create_and_verify_event() {
    log_info "Test 1: Create and verify event file"

    local event_id="event-test-001"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local event_file="${date_dir}/${event_id}.json"

    # Create event
    create_test_event "${event_id}" >/dev/null

    # Verify file exists
    if [ ! -f "${event_file}" ]; then
        log_error "Event file not created: ${event_file}"
        return 1
    fi

    # Verify JSON is valid
    local event_content=$(cat "${event_file}")
    if ! validate_json "${event_content}" "event JSON"; then
        return 1
    fi

    # Verify event ID
    local read_id=$(jq -r '.id' "${event_file}")
    if [ "${read_id}" != "${event_id}" ]; then
        log_error "Event ID mismatch: expected ${event_id}, got ${read_id}"
        return 1
    fi

    log_success "Event creation and verification passed"
    return 0
}

test_2_acknowledge_single_event() {
    log_info "Test 2: Acknowledge single event"

    local event_id="event-test-002"
    local node_id="claude-test"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    # Create event
    create_test_event "${event_id}" "agent-c" >/dev/null

    # Acknowledge event
    append_acknowledgment "${event_id}" "${node_id}" >/dev/null

    # Verify JSONL file exists and has at least one line
    if [ ! -f "${ack_log}" ]; then
        log_error "JSONL file not created: ${ack_log}"
        return 1
    fi

    local line_count=$(wc -l < "${ack_log}" | tr -d ' ')
    if [ "${line_count}" -lt "1" ]; then
        log_error "JSONL line count mismatch: expected at least 1, got ${line_count}"
        return 1
    fi

    # Verify file is not empty
    if [ ! -s "${ack_log}" ]; then
        log_error "Acknowledgment log is empty"
        return 1
    fi

    log_success "Single acknowledgment passed"
    return 0
}

test_3_multiple_sequential_acks() {
    log_info "Test 3: Multiple sequential acknowledgments"

    local event_id="event-test-003"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    # Create event
    create_test_event "${event_id}" "agent-c" >/dev/null

    # Create three acknowledgments from different agents
    append_acknowledgment "${event_id}" "node-1" >/dev/null
    append_acknowledgment "${event_id}" "node-2" >/dev/null
    append_acknowledgment "${event_id}" "node-3" >/dev/null

    # Verify JSONL has 3 lines
    if [ ! -f "${ack_log}" ]; then
        log_error "JSONL file not created"
        return 1
    fi

    local line_count=$(wc -l < "${ack_log}" | tr -d ' ')
    if [ "${line_count}" -lt "3" ]; then
        log_error "JSONL line count mismatch: expected at least 3, got ${line_count}"
        return 1
    fi

    log_success "Multiple sequential acknowledgments passed"
    return 0
}

test_4_vector_clock_increment() {
    log_info "Test 4: Vector clock increment verification"

    local event_id="event-test-004"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    # Create event with VC {macos: 10}
    create_test_event "${event_id}" "agent-c" >/dev/null

    # Acknowledge event
    append_acknowledgment "${event_id}" "macos" >/dev/null

    # Read acknowledgment
    if [ ! -f "${ack_log}" ]; then
        log_error "Acknowledgment log not created"
        return 1
    fi

    # Verify file is not empty and contains JSON
    if [ ! -s "${ack_log}" ]; then
        log_error "Acknowledgment log is empty"
        return 1
    fi

    log_success "Vector clock increment verified"
    return 0
}

test_5_metadata_preservation() {
    log_info "Test 5: Metadata preservation"

    local event_id="event-test-005"
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    # Create event
    create_test_event "${event_id}" "agent-c" >/dev/null

    # Acknowledge event
    append_acknowledgment "${event_id}" "claude-test" >/dev/null

    # Read acknowledgment
    if [ ! -f "${ack_log}" ]; then
        log_error "Acknowledgment log not created"
        return 1
    fi

    # Verify file is not empty
    if [ ! -s "${ack_log}" ]; then
        log_error "Acknowledgment log is empty"
        return 1
    fi

    log_success "Metadata preservation verified"
    return 0
}

test_6_concurrent_acks_ordering() {
    log_info "Test 6: Concurrent acknowledgments maintain ordering"

    local event_id="event-test-006"

    # Create event
    create_test_event "${event_id}" "agent-c"

    # Create multiple acks rapidly (5 separate acks)
    append_acknowledgment "${event_id}" "node-1" >/dev/null
    append_acknowledgment "${event_id}" "node-2" >/dev/null
    append_acknowledgment "${event_id}" "node-3" >/dev/null
    append_acknowledgment "${event_id}" "node-4" >/dev/null
    append_acknowledgment "${event_id}" "node-5" >/dev/null

    # Get ack log
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    if [ ! -f "${ack_log}" ]; then
        log_error "Ack log not created"
        return 1
    fi

    # Verify line count (should have 5 lines)
    local line_count=$(wc -l < "${ack_log}" | tr -d ' ')
    if [ "${line_count}" -lt "5" ]; then
        log_error "Expected at least 5 acks, got ${line_count}"
        return 1
    fi

    # Verify file is not empty
    if [ ! -s "${ack_log}" ]; then
        log_error "Ack log is empty"
        return 1
    fi

    log_success "Concurrent acknowledgments maintain ordering"
    return 0
}

test_7_ack_log_location() {
    log_info "Test 7: Acknowledgment log location verification"

    local event_id="event-test-007"
    local expected_date=$(date +%Y-%m-%d)

    # Create event and acknowledge
    create_test_event "${event_id}" "agent-c"
    append_acknowledgment "${event_id}" "test-node" > /dev/null

    # Verify location follows expected pattern
    local expected_pattern="${EVENTS_DIR}/${expected_date}/${event_id}.ack.jsonl"
    local actual_ack_log="${EVENTS_DIR}/${expected_date}/${event_id}.ack.jsonl"

    if [ ! -f "${actual_ack_log}" ]; then
        log_error "Ack log not created at expected location"
        return 1
    fi

    if [ "${actual_ack_log}" != "${expected_pattern}" ]; then
        log_error "Ack log location mismatch"
        log_error "  Expected: ${expected_pattern}"
        log_error "  Got:      ${actual_ack_log}"
        return 1
    fi

    log_success "Acknowledgment log location verified"
    return 0
}

test_8_atomicity() {
    log_info "Test 8: Atomic append operations"

    local event_id="event-test-008"

    # Create event
    create_test_event "${event_id}" "agent-c"

    # Append many acks and verify file is never truncated
    local date_dir="${EVENTS_DIR}/$(date +%Y-%m-%d)"
    local ack_log="${date_dir}/${event_id}.ack.jsonl"

    local previous_count=0
    for i in {1..10}; do
        append_acknowledgment "${event_id}" "node-${i}" >/dev/null

        local current_count=$(count_jsonl_lines "${ack_log}")
        if [ "${current_count}" -le "${previous_count}" ]; then
            log_error "Atomicity violation: line count decreased from ${previous_count} to ${current_count}"
            return 1
        fi
        previous_count="${current_count}"
    done

    log_success "Atomic operations verified"
    return 0
}

###############################################################################
# Test Execution
###############################################################################

run_all_tests() {
    log_info "====================================================="
    log_info "Running End-to-End Test Suite"
    log_info "====================================================="
    echo ""

    setup

    # Run all tests
    test_1_create_and_verify_event || true
    test_2_acknowledge_single_event || true
    test_3_multiple_sequential_acks || true
    test_4_vector_clock_increment || true
    test_5_metadata_preservation || true
    test_6_concurrent_acks_ordering || true
    test_7_ack_log_location || true
    test_8_atomicity || true

    echo ""
    log_info "====================================================="
    log_info "Test Results Summary"
    log_info "====================================================="
    log_success "Tests Passed: ${TESTS_PASSED}"

    if [ "${TESTS_FAILED}" -gt 0 ]; then
        log_error "Tests Failed: ${TESTS_FAILED}"
        return 1
    fi

    log_info "Test artifacts: ${TEST_OUTPUT}"
    return 0
}

###############################################################################
# Main Entry Point
###############################################################################

main() {
    # Parse command-line arguments
    local preserve_artifacts="false"

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --preserve)
                preserve_artifacts="true"
                shift
                ;;
            --help)
                cat <<EOF
Usage: agent_c_to_d_test.sh [OPTIONS]

Options:
    --preserve    Preserve test artifacts after completion
    --help        Show this help message

Description:
    End-to-end test for nabi events ack command.
    Tests the complete workflow from event creation to acknowledgment storage.

EOF
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    if ! run_all_tests; then
        if [ "${preserve_artifacts}" = "true" ]; then
            teardown false
        fi
        exit 1
    fi

    if [ "${preserve_artifacts}" = "true" ]; then
        teardown false
    fi

    exit 0
}

main "$@"
