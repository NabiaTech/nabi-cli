#!/bin/bash
# Diagnostic script to trace nabi-cli deployment issues
# Checks: build artifacts, symlinks, PATH resolution, binary verification

set -euo pipefail
# Allow commands to fail in conditionals
set +e

echo "🔍 nabi-cli Deployment Diagnostic"
echo "=================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Paths
XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
BUILD_BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
INSTALL_BINARY="${XDG_DATA}/nabi/bin/nabi"
PATH_BINARY="${HOME}/.local/bin/nabi"

echo "📁 Path Resolution"
echo "------------------"
echo "Build location:    ${BUILD_BINARY}"
echo "Install location:  ${INSTALL_BINARY}"
echo "PATH resolves to:  $(which nabi 2>/dev/null || echo 'NOT IN PATH')"
echo ""

# Check build binary
echo "🔨 Build Artifact Check"
echo "----------------------"
if [ -f "${BUILD_BINARY}" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        BUILD_SIZE=$(stat -f%z "${BUILD_BINARY}" 2>/dev/null || echo "0")
        BUILD_TIME=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" "${BUILD_BINARY}" 2>/dev/null || echo "unknown")
        BUILD_MD5=$(md5 -q "${BUILD_BINARY}" 2>/dev/null || echo "unknown")
        BUILD_EPOCH=$(stat -f "%m" "${BUILD_BINARY}" 2>/dev/null || echo "0")
    else
        BUILD_SIZE=$(stat -c%s "${BUILD_BINARY}" 2>/dev/null || echo "0")
        BUILD_TIME=$(stat -c "%y" "${BUILD_BINARY}" 2>/dev/null | cut -d. -f1 || echo "unknown")
        BUILD_MD5=$(md5sum "${BUILD_BINARY}" 2>/dev/null | cut -d' ' -f1 || echo "unknown")
        BUILD_EPOCH=$(stat -c "%Y" "${BUILD_BINARY}" 2>/dev/null || echo "0")
    fi
    echo -e "${GREEN}✓${NC} Build binary exists"
    echo "  Size: ${BUILD_SIZE} bytes"
    echo "  Modified: ${BUILD_TIME}"
    echo "  MD5: ${BUILD_MD5}"

    # Test execution
    if "${BUILD_BINARY}" --version >/dev/null 2>&1; then
        BUILD_VERSION=$("${BUILD_BINARY}" --version 2>&1)
        echo -e "  ${GREEN}✓${NC} Executable: ${BUILD_VERSION}"
    else
        echo -e "  ${RED}✗${NC} Execution failed (exit code: $?)"
    fi
else
    echo -e "${RED}✗${NC} Build binary NOT FOUND"
    echo "  Run: cargo build --release"
fi
echo ""

# Check install binary (symlink target)
echo "📦 Installation Check"
echo "-------------------"
if [ -L "${INSTALL_BINARY}" ]; then
    SYMLINK_TARGET=$(readlink "${INSTALL_BINARY}")
    echo -e "${GREEN}✓${NC} Install location is symlink"
    echo "  Target: ${SYMLINK_TARGET}"

    if [ -f "${SYMLINK_TARGET}" ]; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            INSTALL_SIZE=$(stat -f%z "${SYMLINK_TARGET}" 2>/dev/null)
            INSTALL_TIME=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" "${SYMLINK_TARGET}" 2>/dev/null)
            INSTALL_MD5=$(md5 -q "${SYMLINK_TARGET}" 2>/dev/null)
            INSTALL_EPOCH=$(stat -f "%m" "${SYMLINK_TARGET}" 2>/dev/null)
        else
            INSTALL_SIZE=$(stat -c%s "${SYMLINK_TARGET}" 2>/dev/null)
            INSTALL_TIME=$(stat -c "%y" "${SYMLINK_TARGET}" 2>/dev/null | cut -d. -f1)
            INSTALL_MD5=$(md5sum "${SYMLINK_TARGET}" 2>/dev/null | cut -d' ' -f1)
            INSTALL_EPOCH=$(stat -c "%Y" "${SYMLINK_TARGET}" 2>/dev/null)
        fi
        echo -e "  ${GREEN}✓${NC} Target exists"
        echo "  Size: ${INSTALL_SIZE} bytes"
        echo "  Modified: ${INSTALL_TIME}"
        echo "  MD5: ${INSTALL_MD5}"

        # Compare with build binary
        if [ -f "${BUILD_BINARY}" ]; then
            if [ "${BUILD_MD5}" = "${INSTALL_MD5}" ]; then
                echo -e "  ${GREEN}✓${NC} MD5 matches build binary (code identical)"
            else
                echo -e "  ${YELLOW}⚠${NC} MD5 differs from build binary"
                echo "    This is normal if code signing changed the binary"
            fi

            # Compare timestamps (already set above)
            if [ -n "${INSTALL_EPOCH}" ] && [ -n "${BUILD_EPOCH}" ] && [ "${INSTALL_EPOCH}" != "0" ] && [ "${BUILD_EPOCH}" != "0" ]; then
                if [ "${INSTALL_EPOCH}" -ge "${BUILD_EPOCH}" ]; then
                    echo -e "  ${GREEN}✓${NC} Install timestamp >= build timestamp"
                else
                    echo -e "  ${RED}✗${NC} Install timestamp < build timestamp (STALE!)"
                fi
            else
                echo -e "  ${YELLOW}⚠${NC} Cannot compare timestamps (missing epoch values)"
            fi
        fi

        # Test execution
        if "${SYMLINK_TARGET}" --version >/dev/null 2>&1; then
            INSTALL_VERSION=$("${SYMLINK_TARGET}" --version 2>&1)
            echo -e "  ${GREEN}✓${NC} Executable: ${INSTALL_VERSION}"
        else
            echo -e "  ${RED}✗${NC} Execution failed (exit code: $?)"
        fi
    else
        echo -e "  ${RED}✗${NC} Symlink target does not exist"
    fi
elif [ -f "${INSTALL_BINARY}" ]; then
    echo -e "${GREEN}✓${NC} Install location is regular file"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        INSTALL_MD5=$(md5 -q "${INSTALL_BINARY}" 2>/dev/null)
    else
        INSTALL_MD5=$(md5sum "${INSTALL_BINARY}" 2>/dev/null | cut -d' ' -f1)
    fi
    echo "  MD5: ${INSTALL_MD5}"
else
    echo -e "${RED}✗${NC} Install binary NOT FOUND"
    echo "  Run: just install"
fi
echo ""

# Check PATH resolution
echo "🛤️  PATH Resolution"
echo "-------------------"
WHICH_NABI=$(which nabi 2>/dev/null || echo "")
if [ -n "${WHICH_NABI}" ]; then
    echo -e "${GREEN}✓${NC} nabi found in PATH: ${WHICH_NABI}"

    if [ -f "${WHICH_NABI}" ]; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            PATH_MD5=$(md5 -q "${WHICH_NABI}" 2>/dev/null)
        else
            PATH_MD5=$(md5sum "${WHICH_NABI}" 2>/dev/null | cut -d' ' -f1)
        fi
        echo "  MD5: ${PATH_MD5}"

        # Compare with install binary
        if [ -f "${INSTALL_BINARY}" ] || [ -f "${SYMLINK_TARGET}" ]; then
            COMPARE_TARGET="${SYMLINK_TARGET:-${INSTALL_BINARY}}"
            if [[ "$OSTYPE" == "darwin"* ]]; then
                COMPARE_MD5=$(md5 -q "${COMPARE_TARGET}" 2>/dev/null)
            else
                COMPARE_MD5=$(md5sum "${COMPARE_TARGET}" 2>/dev/null | cut -d' ' -f1)
            fi
            if [ "${PATH_MD5}" = "${COMPARE_MD5}" ]; then
                echo -e "  ${GREEN}✓${NC} PATH binary matches install binary"
            else
                echo -e "  ${RED}✗${NC} PATH binary differs from install binary!"
                echo "    PATH: ${PATH_MD5}"
                echo "    Install: ${COMPARE_MD5}"
            fi
        fi

        # Test execution
        if nabi --version >/dev/null 2>&1; then
            PATH_VERSION=$(nabi --version 2>&1)
            echo -e "  ${GREEN}✓${NC} Executable: ${PATH_VERSION}"
        else
            echo -e "  ${RED}✗${NC} Execution failed (exit code: $?)"
        fi
    fi
else
    echo -e "${RED}✗${NC} nabi NOT in PATH"
    echo "  Add to PATH: export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
echo ""

# Check Cargo cache
echo "🗄️  Cargo Cache Check"
echo "---------------------"
CARGO_TARGET="${XDG_CACHE}/nabi/nabi-cli/target"
if [ -d "${CARGO_TARGET}" ]; then
    TARGET_SIZE=$(du -sh "${CARGO_TARGET}" 2>/dev/null | cut -f1)
    echo "  Cache location: ${CARGO_TARGET}"
    echo "  Cache size: ${TARGET_SIZE}"

    # Check for incremental compilation artifacts
    if [ -d "${CARGO_TARGET}/release/incremental" ]; then
        INCR_COUNT=$(find "${CARGO_TARGET}/release/incremental" -type f 2>/dev/null | wc -l | tr -d ' ')
        echo "  Incremental artifacts: ${INCR_COUNT} files"
        echo -e "  ${YELLOW}💡${NC} Tip: Run 'cargo clean' to clear incremental cache"
    fi
else
    echo -e "  ${YELLOW}⚠${NC} Cargo target directory not found"
fi
echo ""

# Summary
echo "📊 Summary"
echo "---------"
if [ -f "${BUILD_BINARY}" ] && [ -f "${PATH_BINARY}" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        BUILD_EPOCH=$(stat -f "%m" "${BUILD_BINARY}" 2>/dev/null || echo "0")
        PATH_EPOCH=$(stat -f "%m" "${PATH_BINARY}" 2>/dev/null || echo "0")
    else
        BUILD_EPOCH=$(stat -c "%Y" "${BUILD_BINARY}" 2>/dev/null || echo "0")
        PATH_EPOCH=$(stat -c "%Y" "${PATH_BINARY}" 2>/dev/null || echo "0")
    fi

    if [ -n "${PATH_EPOCH}" ] && [ -n "${BUILD_EPOCH}" ] && [ "${PATH_EPOCH}" != "0" ] && [ "${BUILD_EPOCH}" != "0" ]; then
        if [ "${PATH_EPOCH}" -ge "${BUILD_EPOCH}" ]; then
            echo -e "${GREEN}✓${NC} Installation appears up-to-date"
        else
            echo -e "${RED}✗${NC} Installation is STALE (older than build)"
            echo "  Run: just install"
        fi
    else
        echo -e "${YELLOW}⚠${NC} Cannot compare timestamps (using MD5 comparison)"
        BUILD_MD5_FINAL=$(md5 -q "${BUILD_BINARY}" 2>/dev/null || md5sum "${BUILD_BINARY}" 2>/dev/null | cut -d' ' -f1)
        PATH_MD5_FINAL=$(md5 -q "${PATH_BINARY}" 2>/dev/null || md5sum "${PATH_BINARY}" 2>/dev/null | cut -d' ' -f1)
        if [ "${PATH_MD5_FINAL}" = "${BUILD_MD5_FINAL}" ]; then
            echo -e "  ${GREEN}✓${NC} MD5 matches (code identical)"
        else
            echo -e "  ${RED}✗${NC} MD5 differs (code changed)"
        fi
    fi
else
    echo -e "${YELLOW}⚠${NC} Cannot determine status (missing binaries)"
fi
