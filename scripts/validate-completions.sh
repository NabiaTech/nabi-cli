#!/usr/bin/env bash
#
# Completion Composition Validator (Build-Time)
# =============================================
# Fast validation of merged completion without needing Rust compilation.
# Runs as part of: make completions
# Called by: Makefile (lines 77-79)
#
# Checks:
#   1. Both source files exist and are valid zsh
#   2. Merged output is valid zsh
#   3. Dynamic enhancements are properly injected
#   4. No structural corruption
#
# Exit codes:
#   0 = All checks passed
#   1 = Non-critical check failed (warnings)
#   2 = Critical check failed (abort build)

# Note: Using 'set -u' only to avoid exit on pipe failures during validation
set -u

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/.build"

# Files - check both possible locations (justfile uses ~/.zsh, Makefile uses ~/.cache)
STATIC_FILE="$BUILD_DIR/_nabi_static"
DYNAMIC_FILE="$PROJECT_ROOT/contrib/nabi-completions-dynamic.zsh"

# Determine completion file location (try justfile location first, then Makefile location)
if [[ -f "$HOME/.zsh/completions/_nabi" ]]; then
    OUTPUT_FILE="$HOME/.zsh/completions/_nabi"
elif [[ -f "${XDG_CACHE_HOME:-$HOME/.cache}/zsh/completions/_nabi" ]]; then
    OUTPUT_FILE="${XDG_CACHE_HOME:-$HOME/.cache}/zsh/completions/_nabi"
else
    # Default to justfile location if neither exists yet (for fresh builds)
    OUTPUT_FILE="$HOME/.zsh/completions/_nabi"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# State
PASSED=0
FAILED=0
WARNINGS=0

# Helper: Pass/Fail logging
pass() { echo -e "${GREEN}✓${NC} $*"; ((PASSED++)); }
fail() { echo -e "${RED}✗${NC} $*"; ((FAILED++)); }
warn() { echo -e "${YELLOW}⚠${NC} $*"; ((WARNINGS++)); }
info() { echo -e "${BLUE}→${NC} $*"; }

# ─────────────────────────────────────────────────────────────────────────────
# VALIDATION CHECKS
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo "Completion Composition Validator"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Prerequisites
echo "Checking prerequisites..."
for file in "$STATIC_FILE" "$DYNAMIC_FILE" "$OUTPUT_FILE"; do
    if [[ ! -f "$file" ]]; then
        fail "File not found: $file"
        exit 2
    fi
    lines=$(wc -l < "$file")
    info "Found: $(basename "$file") ($lines lines)"
done
pass "All files exist"
echo ""

# CRITICAL CHECKS (abort build if fail)
echo "Critical checks:"

# Check 1: Static syntax
if bash -n "$STATIC_FILE" 2>/dev/null; then
    pass "Static completion has valid zsh syntax"
else
    fail "Static completion has INVALID zsh syntax"
    exit 2
fi

# Check 2: Dynamic syntax
if bash -n "$DYNAMIC_FILE" 2>/dev/null; then
    pass "Dynamic completion has valid zsh syntax"
else
    fail "Dynamic completion has INVALID zsh syntax"
    exit 2
fi

# Check 3: Output syntax
if bash -n "$OUTPUT_FILE" 2>/dev/null; then
    pass "Merged output has valid zsh syntax"
else
    fail "Merged output has INVALID zsh syntax"
    exit 2
fi

# Check 4: Single #compdef line
compdef_count=$(grep -c '^#compdef' "$OUTPUT_FILE" || true)
if [[ $compdef_count -eq 1 ]] && head -1 "$OUTPUT_FILE" | grep -q '^#compdef nabi'; then
    pass "Exactly one #compdef directive (at start)"
else
    fail "Expected 1 #compdef line at start, found $compdef_count"
    exit 2
fi

# Check 5: Dynamic injection marker
if grep -q "Dynamic tmux completion enhancements (injected at build time)" "$OUTPUT_FILE"; then
    pass "Dynamic injection marker present"
else
    fail "Dynamic injection marker MISSING - merge may have failed"
    exit 2
fi

# Check 6: Dynamic override hooks
if grep -q "if (( \$+functions\[_nabi\] ))" "$OUTPUT_FILE"; then
    pass "Dynamic override mechanism intact"
else
    fail "Dynamic override mechanism MISSING - dynamic features won't work"
    exit 2
fi

# Check 7: Key dynamic functions
if grep -q "_nabi_get_tmux_sessions" "$OUTPUT_FILE" && grep -q "_nabi_tmux_send_prompt_pane" "$OUTPUT_FILE"; then
    pass "Dynamic helper functions present"
else
    fail "Dynamic helper functions MISSING"
    exit 2
fi

echo ""
echo "Non-critical checks:"

# Check 8: File size sanity
# Use wc -c for portable file size (avoids custom stat script in ~/.local/bin/stat)
output_size=$(wc -c < "$OUTPUT_FILE")
if [[ $output_size -gt 150000 && $output_size -lt 500000 ]]; then
    pass "Output file size is reasonable ($((output_size / 1024))KB)"
else
    warn "Output file size unusual: $((output_size / 1024))KB (expected 150-500KB)"
fi

# Check 9: No duplicate function definitions
if awk '/^[a-z_]+\(\)/ { if (seen[$0]++) exit 1 }' "$OUTPUT_FILE" 2>/dev/null; then
    pass "No duplicate function definitions"
else
    warn "Possible duplicate function definitions detected"
fi

# Check 10: Static content preserved
if grep -q "autoload -U is-at-least" "$OUTPUT_FILE"; then
    pass "Static completion base preserved"
else
    warn "Static completion base not found in merged file"
fi

# ─────────────────────────────────────────────────────────────────────────────
# SUMMARY
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo "Results: $PASSED passed, $FAILED failed, $WARNINGS warnings"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

if [[ $FAILED -gt 0 ]]; then
    echo -e "${RED}✗ Validation FAILED - build aborted${NC}"
    exit 2
fi

if [[ $WARNINGS -gt 0 ]]; then
    echo -e "${YELLOW}⚠ Validation passed with $WARNINGS warning(s)${NC}"
    exit 0
fi

echo -e "${GREEN}✓ All validation checks PASSED${NC}"
exit 0
