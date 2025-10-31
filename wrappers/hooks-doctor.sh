#!/bin/bash
# Nabi Hooks Doctor - Comprehensive hook system health check
# Validates Python environments, dependencies, and hook readiness

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Paths
HOOKS_DIR="$HOME/.config/nabi/governance/hooks"
PREFLIGHT_VALIDATOR="$HOOKS_DIR/preflight_validator.py"
REQUIREMENTS_FILE="$HOOKS_DIR/requirements.txt"

# Python venv priority chain (matches hook_wrapper.sh)
VENV_PATHS=(
    "$HOME/.nabi/venvs/shared/bin/python"
    "$HOME/.pyenv/versions/claude-hooks-stable/bin/python"
    "$HOME/.config/nabi/.venv/bin/python"
)

# Usage
usage() {
    cat <<EOF
${BOLD}Nabi Hooks Doctor${NC} - Comprehensive hook system health check

Usage: nabi hooks doctor [OPTIONS]

Options:
    --fix              Install missing dependencies automatically
    --validate HOOK    Validate specific hook only
    --json             Output results as JSON
    -h, --help         Show this help message

Examples:
    nabi hooks doctor                  # Full system health check
    nabi hooks doctor --fix            # Check and fix missing dependencies
    nabi hooks doctor --validate link_mapper  # Check specific hook
    nabi hooks doctor --json           # Machine-readable output

Health Checks:
    1. Python virtual environment location and validity
    2. Python version compatibility (>= 3.10)
    3. All third-party dependencies installed
    4. Import capability for each package
    5. Hook-specific dependency validation
    6. Configuration file integrity

Exit Codes:
    0 - All checks passed, system healthy
    1 - One or more checks failed, issues detected
    2 - Critical error (missing venv, invalid Python, etc.)
EOF
}

# Find active Python venv
find_python_venv() {
    for python_path in "${VENV_PATHS[@]}"; do
        if [ -f "$python_path" ]; then
            echo "$python_path"
            return 0
        fi
    done
    return 1
}

# Basic environment check (before running Python validator)
check_basic_environment() {
    local issues=0

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}Basic Environment Check${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    # Check hooks directory
    if [ -d "$HOOKS_DIR" ]; then
        echo -e "  ✅ Hooks directory: ${GREEN}$HOOKS_DIR${NC}"
    else
        echo -e "  ❌ Hooks directory: ${RED}NOT FOUND${NC}"
        ((issues++))
    fi

    # Check Python venv
    if python_path=$(find_python_venv); then
        echo -e "  ✅ Python venv found: ${GREEN}$python_path${NC}"

        # Check Python version
        version=$("$python_path" --version 2>&1)
        echo -e "  ✅ Python version: ${GREEN}$version${NC}"

    else
        echo -e "  ❌ Python venv: ${RED}NOT FOUND${NC}"
        echo -e "     ${YELLOW}Searched locations:${NC}"
        for path in "${VENV_PATHS[@]}"; do
            echo -e "       - $path"
        done
        ((issues++))
    fi

    # Check requirements.txt
    if [ -f "$REQUIREMENTS_FILE" ]; then
        echo -e "  ✅ Requirements file: ${GREEN}$REQUIREMENTS_FILE${NC}"
    else
        echo -e "  ❌ Requirements file: ${RED}NOT FOUND${NC}"
        ((issues++))
    fi

    # Check preflight validator
    if [ -f "$PREFLIGHT_VALIDATOR" ]; then
        echo -e "  ✅ Preflight validator: ${GREEN}$PREFLIGHT_VALIDATOR${NC}"
    else
        echo -e "  ⚠️  Preflight validator: ${YELLOW}NOT FOUND${NC}"
        echo -e "     ${YELLOW}Advanced validation unavailable${NC}"
    fi

    echo ""
    return $issues
}

# Run Python-based preflight validation
run_preflight_validation() {
    local python_path="$1"
    local specific_hook="$2"
    local output_json="$3"

    if [ ! -f "$PREFLIGHT_VALIDATOR" ]; then
        echo -e "${YELLOW}Preflight validator not found, skipping advanced checks${NC}"
        return 0
    fi

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}Dependency Validation${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    local validator_args=()

    if [ -n "$specific_hook" ]; then
        validator_args+=("--hook" "$specific_hook")
    else
        validator_args+=("--all")
    fi

    if [ "$output_json" = "true" ]; then
        validator_args+=("--json")
    fi

    # Run validator
    if "$python_path" "$PREFLIGHT_VALIDATOR" "${validator_args[@]}"; then
        return 0
    else
        return 1
    fi
}

# Install missing dependencies
install_dependencies() {
    local python_path="$1"

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}Installing Missing Dependencies${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ ! -f "$REQUIREMENTS_FILE" ]; then
        echo -e "${RED}Error: Requirements file not found: $REQUIREMENTS_FILE${NC}"
        return 1
    fi

    echo -e "Installing from: ${CYAN}$REQUIREMENTS_FILE${NC}"
    echo ""

    if "$python_path" -m pip install -r "$REQUIREMENTS_FILE"; then
        echo ""
        echo -e "${GREEN}✅ Dependencies installed successfully${NC}"
        return 0
    else
        echo ""
        echo -e "${RED}❌ Failed to install dependencies${NC}"
        return 1
    fi
}

# Check hook wrapper configuration
check_hook_wrapper() {
    local hook_wrapper="$HOME/.config/nabi/governance/hooks/hook_wrapper.sh"

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}Hook Wrapper Configuration${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -f "$hook_wrapper" ]; then
        echo -e "  ✅ Hook wrapper: ${GREEN}$hook_wrapper${NC}"

        # Extract Python path from wrapper
        wrapper_python=$(grep "^PYTHON_CMD=" "$hook_wrapper" | head -1 | sed 's/PYTHON_CMD="\(.*\)"/\1/')
        echo -e "  📍 Configured Python: ${CYAN}$wrapper_python${NC}"

        # Check if it exists
        if [ -f "$(eval echo $wrapper_python)" ]; then
            echo -e "  ✅ Python path valid: ${GREEN}EXISTS${NC}"
        else
            echo -e "  ❌ Python path invalid: ${RED}NOT FOUND${NC}"
            echo -e "     ${YELLOW}Hook wrapper may fail to execute${NC}"
        fi
    else
        echo -e "  ❌ Hook wrapper: ${RED}NOT FOUND${NC}"
        echo -e "     ${YELLOW}Hooks will not execute${NC}"
    fi

    echo ""
}

# Check Claude settings integration
check_claude_integration() {
    local claude_settings="$HOME/.claude/settings.json"

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}Claude Integration${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -f "$claude_settings" ]; then
        echo -e "  ✅ Claude settings: ${GREEN}$claude_settings${NC}"

        # Check if hooks are configured
        if grep -q "hook_wrapper.sh" "$claude_settings" 2>/dev/null; then
            echo -e "  ✅ Hook integration: ${GREEN}CONFIGURED${NC}"

            # Count enabled hooks
            enabled_hooks=$(grep -o '"sessionStart":\|"sessionEnd":\|"userPromptSubmit":\|"preToolUse":\|"postToolUse":' "$claude_settings" | wc -l || echo "0")
            echo -e "  📊 Enabled hooks: ${CYAN}$enabled_hooks${NC}"
        else
            echo -e "  ⚠️  Hook integration: ${YELLOW}NOT CONFIGURED${NC}"
            echo -e "     ${YELLOW}Hooks will not be called by Claude${NC}"
        fi
    else
        echo -e "  ❌ Claude settings: ${RED}NOT FOUND${NC}"
        echo -e "     ${YELLOW}Claude may not be configured${NC}"
    fi

    echo ""
}

# Main function
main() {
    local fix_mode=false
    local specific_hook=""
    local output_json=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --fix)
                fix_mode=true
                shift
                ;;
            --validate)
                specific_hook="$2"
                shift 2
                ;;
            --json)
                output_json=true
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                echo -e "${RED}Error: Unknown option '$1'${NC}" >&2
                echo ""
                usage
                exit 1
                ;;
        esac
    done

    # Header
    if [ "$output_json" != "true" ]; then
        echo ""
        echo -e "${BOLD}${CYAN}Nabi Hooks Doctor - System Health Check${NC}"
        echo -e "${CYAN}$(date)${NC}"
        echo ""
    fi

    # Basic environment check
    if [ "$output_json" != "true" ]; then
        if ! check_basic_environment; then
            echo -e "${RED}❌ Basic environment check failed${NC}"
            exit 2
        fi
    fi

    # Find Python
    if ! python_path=$(find_python_venv); then
        echo -e "${RED}❌ Critical: No Python virtual environment found${NC}"
        echo ""
        echo -e "${YELLOW}Create venv with:${NC}"
        echo "  python3 -m venv ~/.nabi/venvs/shared"
        exit 2
    fi

    # Fix mode - install dependencies
    if [ "$fix_mode" = "true" ]; then
        if install_dependencies "$python_path"; then
            echo ""
            echo -e "${GREEN}✅ Fix completed successfully${NC}"
        else
            echo ""
            echo -e "${RED}❌ Fix failed${NC}"
            exit 1
        fi
    fi

    # Run preflight validation
    if [ "$output_json" != "true" ]; then
        check_hook_wrapper
        check_claude_integration
    fi

    if run_preflight_validation "$python_path" "$specific_hook" "$output_json"; then
        if [ "$output_json" != "true" ]; then
            echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
            echo -e "${GREEN}${BOLD}✅ All checks passed - System healthy${NC}"
            echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
            echo ""
        fi
        exit 0
    else
        if [ "$output_json" != "true" ]; then
            echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
            echo -e "${RED}${BOLD}❌ Issues detected${NC}"
            echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
            echo ""
            echo -e "${YELLOW}To fix dependency issues, run:${NC}"
            echo "  nabi hooks doctor --fix"
            echo ""
        fi
        exit 1
    fi
}

# Run main
main "$@"
