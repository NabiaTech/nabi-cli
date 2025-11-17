#!/bin/bash
# Test script to verify Cargo rebuilds on source changes
# Makes a test change, rebuilds, and verifies the change is in the binary

set -e

echo "🧪 Testing Cargo Rebuild Behavior"
echo "=================================="
echo ""

# Paths
SRC_FILE="src/main.rs"
BUILD_BINARY="${XDG_CACHE_HOME:-$HOME/.cache}/nabi/nabi-cli/target/release/nabi"

# Backup original
echo "📋 Step 1: Backup original source"
BACKUP_FILE="${SRC_FILE}.test-backup"
cp "${SRC_FILE}" "${BACKUP_FILE}"
echo "  ✓ Backed up to ${BACKUP_FILE}"
echo ""

# Make a test change (add a comment)
echo "📝 Step 2: Making test change to source"
TEST_MARKER="// TEST_REBUILD_MARKER_$(date +%s)"
if grep -q "TEST_REBUILD_MARKER" "${SRC_FILE}"; then
    echo "  ⚠️  Test marker already exists, removing old one"
    sed -i '' '/TEST_REBUILD_MARKER/d' "${SRC_FILE}"
fi
# Add marker after the first use statement
sed -i '' '/^use anyhow/a\
'"${TEST_MARKER}"'
' "${SRC_FILE}"
echo "  ✓ Added test marker: ${TEST_MARKER}"
echo ""

# Check if marker is in source
if grep -q "TEST_REBUILD_MARKER" "${SRC_FILE}"; then
    echo "  ✓ Test marker confirmed in source"
else
    echo "  ✗ Test marker NOT found in source!"
    exit 1
fi
echo ""

# Rebuild
echo "🔨 Step 3: Rebuilding..."
cargo build --release 2>&1 | tail -3
echo ""

# Check if marker is in binary (as a string)
echo "🔍 Step 4: Checking if marker is in binary"
if strings "${BUILD_BINARY}" | grep -q "TEST_REBUILD_MARKER"; then
    echo "  ✓ Test marker found in binary (rebuild successful)"
else
    echo "  ✗ Test marker NOT found in binary (rebuild may have failed or cached)"
    echo "  Checking binary timestamp..."
    ls -lh "${BUILD_BINARY}"
fi
echo ""

# Restore original
echo "🔄 Step 5: Restoring original source"
mv "${BACKUP_FILE}" "${SRC_FILE}"
echo "  ✓ Restored original"
echo ""

# Rebuild again to restore
echo "🔨 Step 6: Rebuilding to restore original state"
cargo build --release 2>&1 | tail -3
echo ""

echo "✅ Test complete"
