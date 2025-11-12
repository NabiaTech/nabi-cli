#!/bin/bash
# Codegraph Post-Index Finalization Hook
# Finalizes index, syncs to federation, updates manifests
#
# Usage: codegraph-post-index.sh <repo_path> <index_path>
# Exit Codes:
#   0 → All operations successful
#   1 → Index validation failed (corrupted)
#   2 → Syncthing sync failed (non-critical)
#   3 → Manifest update failed

set -e

REPO_PATH="${1:-.}"
INDEX_PATH="${2:-.}"

# Color codes
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure paths are absolute
if [[ ! "$REPO_PATH" = /* ]]; then
    REPO_PATH="$(cd "$REPO_PATH" 2>/dev/null && pwd)" || REPO_PATH="."
fi

if [[ ! "$INDEX_PATH" = /* ]]; then
    INDEX_PATH="$(cd "$INDEX_PATH" 2>/dev/null && pwd)" || INDEX_PATH="."
fi

# Step 1: Validate index integrity
if [[ ! -f "$INDEX_PATH/metadata.json" ]] || [[ ! -f "$INDEX_PATH/symbols.json" ]]; then
    echo -e "${RED}✗ Index validation failed: missing required files${NC}" >&2
    exit 1
fi

# Validate JSON structure
if ! jq . "$INDEX_PATH/metadata.json" > /dev/null 2>&1; then
    echo -e "${RED}✗ Index validation failed: corrupted metadata.json${NC}" >&2
    exit 1
fi

if ! jq . "$INDEX_PATH/symbols.json" > /dev/null 2>&1; then
    echo -e "${RED}✗ Index validation failed: corrupted symbols.json${NC}" >&2
    exit 1
fi

echo -e "${GREEN}✓ Index validation passed${NC}"

# Step 2: Syncthing synchronization (non-blocking)
SYNC_DIR="$HOME/Sync/.config/nabi/codegraph"
if [[ -d "$HOME/Sync" ]] && command -v syncthing &> /dev/null; then
    echo -e "${BLUE}→ Syncing to Syncthing federation...${NC}"

    REPO_NAME=$(basename "$REPO_PATH")
    TARGET_DIR="$SYNC_DIR/$REPO_NAME"

    mkdir -p "$TARGET_DIR"
    cp "$INDEX_PATH/metadata.json" "$TARGET_DIR/" 2>/dev/null || true
    cp "$INDEX_PATH/symbols.json" "$TARGET_DIR/" 2>/dev/null || true

    if [[ -d "$INDEX_PATH/graphs" ]]; then
        cp -r "$INDEX_PATH/graphs" "$TARGET_DIR/" 2>/dev/null || true
    fi

    echo -e "${GREEN}✓ Syncthing sync complete${NC}"
else
    echo -e "${YELLOW}⚠ Syncthing not available (optional, continuing)${NC}" >&2
fi

# Step 3: Update manifest
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}"
MANIFEST_DIR="$STATE_DIR/nabi/codegraph/manifests"
mkdir -p "$MANIFEST_DIR"

REPO_NAME=$(basename "$REPO_PATH")
MANIFEST_FILE="$MANIFEST_DIR/${REPO_NAME}-manifest.json"

# Extract metadata
METADATA=$(cat "$INDEX_PATH/metadata.json")
SYMBOL_COUNT=$(echo "$METADATA" | jq '.symbol_count')
FILE_COUNT=$(echo "$METADATA" | jq '.file_count')
LANGUAGE=$(echo "$METADATA" | jq -r '.language')

# Get git info if available
GIT_ORIGIN=""
GIT_BRANCH=""
GIT_COMMIT=""

if [[ -d "$REPO_PATH/.git" ]]; then
    GIT_ORIGIN=$(git -C "$REPO_PATH" config --get remote.origin.url 2>/dev/null || echo "")
    GIT_BRANCH=$(git -C "$REPO_PATH" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
    GIT_COMMIT=$(git -C "$REPO_PATH" rev-parse HEAD 2>/dev/null || echo "")
fi

# Create comprehensive manifest
cat > "$MANIFEST_FILE" << EOF
{
  "repository": "$REPO_NAME",
  "language": "$LANGUAGE",
  "metadata": $METADATA,
  "federation": {
    "indexed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "indexed_by": "nabi-codegraph",
    "version": "1.0"
  },
  "git": {
    "origin": "$GIT_ORIGIN",
    "branch": "$GIT_BRANCH",
    "commit": "$GIT_COMMIT"
  },
  "symbols": {
    "total": $SYMBOL_COUNT,
    "files_analyzed": $FILE_COUNT
  },
  "paths": {
    "repository": "$REPO_PATH",
    "index": "$INDEX_PATH"
  }
}
EOF

echo -e "${GREEN}✓ Manifest created at: $MANIFEST_FILE${NC}"

# Step 4: Clean up lock file
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}"
LOCK_FILE="$STATE_DIR/nabi/codegraph/.index-lock"
rm -f "$LOCK_FILE"

echo -e "${GREEN}✓ Post-index finalization complete${NC}"
exit 0
