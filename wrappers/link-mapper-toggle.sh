#!/bin/bash
# Link Mapper CLI Toggle Script
# Enable/disable link-mapper hooks via feature flags

set -euo pipefail

# XDG paths
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
FEATURE_FLAGS_PATH="$XDG_CONFIG_HOME/nabi/link-mapper/feature_flags.toml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Usage information
usage() {
    cat <<EOF
Usage: nabi link-mapper toggle [COMMAND]

Commands:
    on      Enable all link-mapper hooks
    off     Disable all link-mapper hooks
    status  Show current status (default)

Examples:
    nabi link-mapper toggle on       # Enable all hooks
    nabi link-mapper toggle off      # Disable all hooks
    nabi link-mapper toggle status   # Show current status
    nabi link-mapper toggle          # Show current status

Configuration file: $FEATURE_FLAGS_PATH
EOF
}

# Check if feature flags file exists
check_config_exists() {
    if [ ! -f "$FEATURE_FLAGS_PATH" ]; then
        echo -e "${RED}Error: Feature flags file not found${NC}" >&2
        echo "Expected location: $FEATURE_FLAGS_PATH" >&2
        exit 1
    fi
}

# Get current status of a flag
get_flag_value() {
    local flag_name="$1"
    check_config_exists
    grep "^${flag_name} = " "$FEATURE_FLAGS_PATH" | sed 's/.*= \([^ ]*\).*/\1/'
}

# Set flag value
set_flag_value() {
    local flag_name="$1"
    local new_value="$2"

    check_config_exists

    # Use sed to edit the TOML file
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS sed requires backup extension
        sed -i '' "s/^${flag_name} = .*/${flag_name} = ${new_value}/" "$FEATURE_FLAGS_PATH"
    else
        # Linux sed
        sed -i "s/^${flag_name} = .*/${flag_name} = ${new_value}/" "$FEATURE_FLAGS_PATH"
    fi
}

# Show current status
show_status() {
    check_config_exists

    echo -e "${BLUE}Link Mapper Status${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Master toggle
    enabled=$(get_flag_value "enabled")
    if [ "$enabled" = "true" ]; then
        echo -e "Master Toggle:           ${GREEN}ENABLED${NC}"
    else
        echo -e "Master Toggle:           ${RED}DISABLED${NC}"
    fi

    echo ""
    echo -e "${BLUE}Hook Status:${NC}"

    # Individual hooks
    session_start=$(get_flag_value "session_start_integration")
    user_prompt=$(get_flag_value "user_prompt_validation")
    pre_tool=$(get_flag_value "pre_tool_protection")
    post_tool=$(get_flag_value "post_tool_detection")

    print_hook_status "SessionStart" "$session_start"
    print_hook_status "UserPromptSubmit" "$user_prompt"
    print_hook_status "PreToolUse" "$pre_tool"
    print_hook_status "PostToolUse" "$post_tool"

    echo ""
    echo -e "${BLUE}Federation:${NC}"
    federation=$(get_flag_value "federation_events")
    print_hook_status "Event Logging" "$federation"

    echo ""
    echo -e "${BLUE}Configuration:${NC}"
    echo "Config File: $FEATURE_FLAGS_PATH"

    # Check for state directory
    STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/nabi/link-mapper"
    if [ -d "$STATE_DIR" ]; then
        echo -e "State Dir:   ${GREEN}$STATE_DIR${NC}"

        # Check for active config
        if [ -f "$STATE_DIR/active_config.json" ]; then
            timestamp=$(grep '"timestamp"' "$STATE_DIR/active_config.json" | sed 's/.*": "\(.*\)".*/\1/')
            echo -e "Last Active: ${GREEN}$timestamp${NC}"
        fi

        # Check for errors
        if [ -f "$STATE_DIR/error_count.json" ]; then
            errors=$(grep '"consecutive_errors"' "$STATE_DIR/error_count.json" | sed 's/.*: \([0-9]*\).*/\1/')
            if [ "$errors" -gt 0 ]; then
                echo -e "Errors:      ${RED}$errors consecutive errors${NC}"
            else
                echo -e "Errors:      ${GREEN}None${NC}"
            fi
        fi
    else
        echo -e "State Dir:   ${YELLOW}Not created yet${NC}"
    fi
}

# Helper to print hook status
print_hook_status() {
    local name="$1"
    local value="$2"

    printf "  %-20s " "$name:"
    if [ "$value" = "true" ]; then
        echo -e "${GREEN}ENABLED${NC}"
    else
        echo -e "${RED}DISABLED${NC}"
    fi
}

# Pre-flight dependency check
preflight_check() {
    local preflight_validator="$HOME/.config/nabi/governance/hooks/preflight_validator.py"

    # Find Python venv (same priority as hook_wrapper.sh)
    local python_path=""
    for venv_path in \
        "$HOME/.nabi/venvs/shared/bin/python" \
        "$HOME/.pyenv/versions/claude-hooks-stable/bin/python" \
        "$HOME/.config/nabi/.venv/bin/python"; do
        if [ -f "$venv_path" ]; then
            python_path="$venv_path"
            break
        fi
    done

    # Critical check: Python venv exists
    if [ -z "$python_path" ]; then
        echo -e "${RED}❌ Pre-flight check FAILED${NC}" >&2
        echo -e "${RED}No Python virtual environment found${NC}" >&2
        echo "" >&2
        echo -e "${YELLOW}Create venv with:${NC}" >&2
        echo "  python3 -m venv ~/.nabi/venvs/shared" >&2
        echo "  ~/.nabi/venvs/shared/bin/pip install -r ~/.config/nabi/governance/hooks/requirements.txt" >&2
        return 1
    fi

    # If preflight validator exists, use it
    if [ -f "$preflight_validator" ]; then
        echo -e "${BLUE}Running pre-flight dependency check...${NC}"

        # Run validator for link_mapper hook
        if "$python_path" "$preflight_validator" --hook link_mapper >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Pre-flight check passed${NC}"
            return 0
        else
            echo -e "${RED}❌ Pre-flight check FAILED${NC}" >&2
            echo "" >&2
            echo -e "${YELLOW}Missing dependencies detected. Running detailed check:${NC}" >&2
            "$python_path" "$preflight_validator" --hook link_mapper >&2
            echo "" >&2
            echo -e "${YELLOW}To install missing dependencies:${NC}" >&2
            echo "  nabi hooks doctor --fix" >&2
            echo "" >&2
            echo -e "${YELLOW}Or install manually:${NC}" >&2
            echo "  $python_path -m pip install -r ~/.config/nabi/governance/hooks/requirements.txt" >&2
            return 1
        fi
    else
        # Fallback: Basic import test
        echo -e "${YELLOW}⚠️  Preflight validator not found, running basic checks...${NC}"

        local deps_ok=true
        for dep in tomli jsonschema toml; do
            if ! "$python_path" -c "import $dep" 2>/dev/null; then
                echo -e "${RED}❌ Missing dependency: $dep${NC}" >&2
                deps_ok=false
            fi
        done

        if [ "$deps_ok" = "false" ]; then
            echo "" >&2
            echo -e "${YELLOW}To install missing dependencies:${NC}" >&2
            echo "  $python_path -m pip install tomli jsonschema toml" >&2
            return 1
        fi

        echo -e "${GREEN}✓ Basic dependency check passed${NC}"
        return 0
    fi
}

# Enable all hooks
enable_all() {
    check_config_exists

    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}Enabling link-mapper hooks...${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    # Pre-flight safety check
    if ! preflight_check; then
        echo ""
        echo -e "${RED}❌ Enable aborted - dependency check failed${NC}" >&2
        echo -e "${YELLOW}Fix dependencies before enabling hooks to prevent system failures${NC}" >&2
        exit 1
    fi

    echo ""
    echo -e "${GREEN}Updating feature flags...${NC}"

    set_flag_value "enabled" "true"
    set_flag_value "session_start_integration" "true"
    set_flag_value "federation_events" "true"

    echo -e "${GREEN}✓ Link-mapper enabled${NC}"
    echo ""
    echo "Changes will take effect in the next Claude session."
    echo ""
    show_status
}

# Disable all hooks
disable_all() {
    check_config_exists

    echo -e "${YELLOW}Disabling link-mapper hooks...${NC}"

    set_flag_value "enabled" "false"

    echo -e "${YELLOW}✓ Link-mapper disabled${NC}"
    echo ""
    echo "Changes will take effect in the next Claude session."
    echo ""
    show_status
}

# Main command dispatcher
main() {
    local command="${1:-status}"

    case "$command" in
        on|enable)
            enable_all
            ;;
        off|disable)
            disable_all
            ;;
        status)
            show_status
            ;;
        -h|--help|help)
            usage
            ;;
        *)
            echo -e "${RED}Error: Unknown command '$command'${NC}" >&2
            echo ""
            usage
            exit 1
            ;;
    esac
}

# Run main with all arguments
main "$@"
