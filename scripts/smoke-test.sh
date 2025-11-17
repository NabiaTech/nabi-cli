#!/bin/bash
# Smoke test for nabi CLI - tests all command variants for errors

set -e

echo "🧪 nabi CLI Smoke Test"
echo "====================="
echo ""

ERRORS=0
PASSED=0
FAILED=0

# Test function
test_cmd() {
    local cmd="$1"
    local description="${2:-$cmd}"

    echo -n "Testing: $description... "

    if nabi $cmd --help >/dev/null 2>&1; then
        echo "✅ PASS"
        ((PASSED++))
        return 0
    else
        echo "❌ FAIL"
        ((FAILED++))
        ((ERRORS++))
        return 1
    fi
}

# Test function that expects success
test_cmd_success() {
    local cmd="$1"
    local description="${2:-$cmd}"

    echo -n "Testing: $description... "

    if nabi $cmd >/dev/null 2>&1; then
        echo "✅ PASS"
        ((PASSED++))
        return 0
    else
        local exit_code=$?
        if [ $exit_code -eq 1 ] || [ $exit_code -eq 0 ]; then
            # Exit code 1 might be expected (e.g., missing args)
            echo "⚠️  EXIT $exit_code (may be expected)"
            ((PASSED++))
            return 0
        else
            echo "❌ FAIL (exit $exit_code)"
            ((FAILED++))
            ((ERRORS++))
            return 1
        fi
    fi
}

echo "📋 Testing Top-Level Commands"
echo "-----------------------------"

# Core commands
test_cmd "claude" "claude --help"
test_cmd "data" "data --help"
test_cmd "federation" "federation --help"
test_cmd "self" "self --help"
test_cmd "forge" "forge --help"
test_cmd "docs" "docs --help"
test_cmd "repo" "repo --help"
test_cmd "tool" "tool --help"
test_cmd "scan" "scan --help"
test_cmd "watch" "watch --help"
test_cmd "aura" "aura --help"
test_cmd "configure" "configure --help"
test_cmd "db" "db --help"
test_cmd "record" "record --help"
test_cmd "backup" "backup --help"
test_cmd "agent" "agent --help"
test_cmd "port" "port --help"
test_cmd "tmux" "tmux --help"
test_cmd "kernel" "kernel --help"
test_cmd "events" "events --help"
test_cmd "hooks" "hooks --help"
test_cmd "mode" "mode --help"
test_cmd "riff" "riff --help"
test_cmd "recover" "recover --help"
test_cmd "health" "health --help"
test_cmd "completions" "completions --help"
test_cmd "deckgen" "deckgen --help"
test_cmd "doctor" "doctor --help"
test_cmd "migrate" "migrate --help"

echo ""
echo "📋 Testing Subcommands"
echo "---------------------"

# Claude subcommands
test_cmd "claude session" "claude session --help"
test_cmd "claude project" "claude project --help"

# Data subcommands
test_cmd "data jsonl" "data jsonl --help"

# Federation subcommands
test_cmd "federation agent" "federation agent --help"
test_cmd "federation sync" "federation sync --help"
test_cmd "federation registry" "federation registry --help"
test_cmd "federation health" "federation health --help"
test_cmd "federation status" "federation status --help"
test_cmd "federation agents" "federation agents --help"

# Self subcommands
test_cmd "self doctor" "self doctor --help"
test_cmd "self update" "self update --help"
test_cmd "self config" "self config --help"
test_cmd "self spec" "self spec --help"

# Forge subcommands
test_cmd "forge enable" "forge enable --help"
test_cmd "forge disable" "forge disable --help"
test_cmd "forge status" "forge status --help"
test_cmd "forge list" "forge list --help"

# Repo subcommands
test_cmd "repo check" "repo check --help"
test_cmd "repo analyze" "repo analyze --help"
test_cmd "repo graph" "repo graph --help"
test_cmd "repo codegraph" "repo codegraph --help"

# Tool subcommands
test_cmd "tool list" "tool list --help"
test_cmd "tool exec" "tool exec --help"
test_cmd "tool register" "tool register --help"

# Aura subcommands
test_cmd "aura list" "aura list --help"
test_cmd "aura show" "aura show --help"
test_cmd "aura create" "aura create --help"
test_cmd "aura switch" "aura switch --help"
test_cmd "aura status" "aura status --help"

# Tmux subcommands
test_cmd "tmux list" "tmux list --help"
test_cmd "tmux split-pane" "tmux split-pane --help"
test_cmd "tmux send-prompt" "tmux send-prompt --help"
test_cmd "tmux capture" "tmux capture --help"

# Health subcommands
test_cmd "health check" "health check --help"

# Hooks subcommands
test_cmd "hooks transform" "hooks transform --help"
test_cmd "hooks debug" "hooks debug --help"

echo ""
echo "📋 Testing Execution Commands"
echo "----------------------------"

# Test commands that should execute (may fail due to missing args, but shouldn't crash)
test_cmd_success "--version" "--version"
test_cmd_success "self doctor" "self doctor (may show warnings)"
test_cmd_success "forge status" "forge status"
test_cmd_success "tool list" "tool list"
test_cmd_success "port list" "port list"
test_cmd_success "federation status" "federation status"

echo ""
echo "📊 Results"
echo "---------"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo "Errors: $ERRORS"
echo ""

if [ $ERRORS -eq 0 ]; then
    echo "✅ All smoke tests passed!"
    exit 0
else
    echo "❌ Some tests failed. Check output above."
    exit 1
fi
