#!/bin/bash
# Codegraph Pre-Index Validation Hook
# Validates environment before committing compute resources to indexing
#
# Usage: codegraph-pre-index.sh <repo_path> <language>
# Exit Codes:
#   0 → All checks passed, proceed with indexing
#   1 → Repository invalid
#   2 → Parser tool missing
#   3 → Insufficient disk space (need 100MB+)
#   4 → Index already in progress

set -e

REPO_PATH="${1:-.}"
LANGUAGE="${2:-auto}"

# Color codes
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Ensure repo path is absolute
if [[ ! "$REPO_PATH" = /* ]]; then
    REPO_PATH="$(cd "$REPO_PATH" 2>/dev/null && pwd)" || {
        echo -e "${RED}✗ Invalid repository path${NC}" >&2
        exit 1
    }
fi

# Check 1: Valid directory (must exist and be readable)
if [[ ! -d "$REPO_PATH" ]]; then
    echo -e "${RED}✗ Directory not found: $REPO_PATH${NC}" >&2
    exit 1
fi

if [[ ! -r "$REPO_PATH" ]]; then
    echo -e "${RED}✗ Directory not readable: $REPO_PATH${NC}" >&2
    exit 1
fi

# Check 2: Disk space (minimum 100MB free)
DISK_FREE=$(df "$REPO_PATH" | awk 'NR==2 {print $4}')
MIN_DISK_KB=$((100 * 1024))  # 100MB in KB

if [[ $DISK_FREE -lt $MIN_DISK_KB ]]; then
    echo -e "${RED}✗ Insufficient disk space: ${DISK_FREE}KB available, need ${MIN_DISK_KB}KB${NC}" >&2
    exit 3
fi

# Check 3: No concurrent indexing (via lock file)
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}"
LOCK_FILE="$STATE_DIR/nabi/codegraph/.index-lock"
mkdir -p "$(dirname "$LOCK_FILE")"

if [[ -f "$LOCK_FILE" ]]; then
    LOCK_PID=$(cat "$LOCK_FILE" 2>/dev/null)
    # Check if process still running
    if kill -0 "$LOCK_PID" 2>/dev/null; then
        echo -e "${YELLOW}⚠ Indexing already in progress (PID: $LOCK_PID)${NC}" >&2
        exit 4
    else
        rm -f "$LOCK_FILE"
    fi
fi

# Check 4: Git metadata (optional - only if repo is a git repo)
if [[ -d "$REPO_PATH/.git" ]]; then
    # Warn if repository has uncommitted changes
    if [[ -n $(git -C "$REPO_PATH" status -s 2>/dev/null) ]]; then
        echo -e "${YELLOW}⚠ Repository has uncommitted changes${NC}" >&2
        # Non-blocking - continue anyway
    fi

    # Fail if in rebase/merge state
    if [[ -d "$REPO_PATH/.git/rebase-merge" ]] || [[ -d "$REPO_PATH/.git/rebase-apply" ]]; then
        echo -e "${RED}✗ Repository is in rebase/merge state${NC}" >&2
        exit 1
    fi
else
    echo -e "${YELLOW}⚠ Not a git repository (proceeding with directory indexing)${NC}" >&2
fi

# All checks passed - create lock file
echo $$ > "$LOCK_FILE"

echo -e "${GREEN}✓ Pre-index validation passed${NC}"
exit 0
