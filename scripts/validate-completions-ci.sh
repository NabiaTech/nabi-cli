#!/bin/bash
#
# CI Validation Script for Nabi CLI Completions
# =============================================
#
# This script validates that shell completions work correctly
# and can be run as part of CI/CD pipelines.
#
# Usage: ./validate-completions-ci.sh [--verbose]
#
# Exit codes:
#   0 = All validations passed
#   1 = Validation failed
#   2 = Build/Setup failed

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VERBOSE="${VERBOSE:-false}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}ℹ${NC} $*" >&2; }
log_success() { echo -e "${GREEN}✓${NC} $*" >&2; }
log_warning() { echo -e "${YELLOW}⚠${NC} $*" >&2; }
log_error() { echo -e "${RED}✗${NC} $*" >&2; }

if [[ "$1" == "--verbose" ]]; then
    VERBOSE=true
fi

if [[ "$VERBOSE" == "true" ]]; then
    log_info "Running in verbose mode"
fi

# Check if nabi binary exists
check_binary() {
    local binary_path
    if [[ -n "${NABI_BINARY_PATH:-}" ]]; then
        binary_path="$NABI_BINARY_PATH"
    else
        # Use XDG cache location
        local xdg_cache="${XDG_CACHE_HOME:-$HOME/.cache}"
        binary_path="${xdg_cache}/nabi/nabi-cli/target/release/nabi"
    fi

    if [[ ! -x "$binary_path" ]]; then
        log_error "Nabi binary not found or not executable: $binary_path"
        log_info "Make sure to build the project first: just build"
        return 1
    fi

    echo "$binary_path"
}

# Validate completion generation for a shell
validate_completion_generation() {
    local shell="$1"
    local binary="$2"

    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Testing $shell completion generation..."
    fi

    # Generate completion to temp file
    local temp_file
    temp_file=$(mktemp)

    if ! "$binary" completions "$shell" --output "$temp_file" >/dev/null 2>&1; then
        log_error "$shell completion generation failed"
        rm -f "$temp_file"
        return 1
    fi

    # Check file was created and has content
    if [[ ! -s "$temp_file" ]]; then
        log_error "$shell completion file is empty"
        rm -f "$temp_file"
        return 1
    fi

    # Basic content validation
    local file_size
    file_size=$(wc -c < "$temp_file")

    if [[ $file_size -lt 1000 ]]; then
        log_error "$shell completion file too small (${file_size} bytes)"
        rm -f "$temp_file"
        return 1
    fi

    # Check for expected content markers
    case "$shell" in
        zsh)
            if ! grep -q "#compdef nabi" "$temp_file"; then
                log_error "$shell completion missing #compdef directive"
                rm -f "$temp_file"
                return 1
            fi
            ;;
        bash)
            if ! grep -q "complete.*-F" "$temp_file"; then
                log_error "$shell completion missing complete directive"
                rm -f "$temp_file"
                return 1
            fi
            ;;
    esac

    # Check for dynamic completion markers
    if ! grep -q "Dynamic tool completion function" "$temp_file"; then
        log_error "$shell completion missing dynamic tool completion"
        rm -f "$temp_file"
        return 1
    fi

    rm -f "$temp_file"

    if [[ "$VERBOSE" == "true" ]]; then
        log_success "$shell completion validation passed"
    fi

    return 0
}

# Test dynamic completion functionality
test_dynamic_completion() {
    local binary="$1"

    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Testing dynamic tool completion..."
    fi

    # Generate zsh completion and check it contains tool discovery
    local temp_file
    temp_file=$(mktemp)

    if ! "$binary" completions zsh --output "$temp_file" >/dev/null 2>&1; then
        log_error "Failed to generate completion for dynamic test"
        rm -f "$temp_file"
        return 1
    fi

    # Check for tool list command in dynamic completion
    if ! grep -q "nabi tool list --format=json" "$temp_file"; then
        log_error "Dynamic completion missing tool list command"
        rm -f "$temp_file"
        return 1
    fi

    # Check for jq processing
    if ! grep -q "jq -r" "$temp_file"; then
        log_error "Dynamic completion missing jq processing"
        rm -f "$temp_file"
        return 1
    fi

    rm -f "$temp_file"

    if [[ "$VERBOSE" == "true" ]]; then
        log_success "Dynamic completion validation passed"
    fi

    return 0
}

# Main validation logic
main() {
    log_info "Starting Nabi CLI completion validation..."

    # Find binary
    local binary
    if ! binary=$(check_binary); then
        exit 2
    fi

    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Using binary: $binary"
    fi

    # Test basic functionality
    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Testing basic completion command..."
    fi

    if ! "$binary" completions --help >/dev/null 2>&1; then
        log_error "Basic completions command failed"
        exit 1
    fi

    # Validate completion generation for supported shells
    local shells=("zsh" "bash")
    local failed_shells=()

    for shell in "${shells[@]}"; do
        if ! validate_completion_generation "$shell" "$binary"; then
            failed_shells+=("$shell")
        fi
    done

    # Test dynamic completion
    if ! test_dynamic_completion "$binary"; then
        log_error "Dynamic completion test failed"
        exit 1
    fi

    # Report results
    if [[ ${#failed_shells[@]} -eq 0 ]]; then
        log_success "All completion validations passed!"
        exit 0
    else
        log_error "Completion validation failed for shells: ${failed_shells[*]}"
        exit 1
    fi
}

# Run main function
main "$@"
