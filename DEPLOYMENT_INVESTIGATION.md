# nabi-cli Deployment Mechanism Investigation

**Date**: 2024-11-17
**Issue**: Binary doesn't reflect source code changes despite successful compilation and installation

---

## 🔍 Current State Analysis

### Deployment Flow

```
Source: ~/nabia/core/nabi-cli/src/
  ↓
Build:  ~/.cache/nabi/nabi-cli/target/release/nabi  (XDG_CACHE_HOME)
  ↓
Install: ~/.local/share/nabi/bin/nabi  (symlink)
  ↓
Target: ~/.local/bin/nabi  (actual binary, in PATH)
```

### Key Findings

1. **Symlink Chain**:
   - `~/.local/share/nabi/bin/nabi` → symlink to `~/.local/bin/nabi`
   - `which nabi` resolves to `~/.local/bin/nabi` (the actual binary)

2. **Installation Process**:
   - `just install` copies build binary to `~/.local/share/nabi/bin/nabi`
   - Since this is a symlink, `cp` follows it and writes to `~/.local/bin/nabi`
   - Binary is then code-signed (changes MD5 hash)

3. **Current Status** (from diagnostic):
   - ✅ Build binary exists and is executable
   - ✅ Install binary exists and matches build binary (MD5 identical before signing)
   - ✅ PATH resolution correct
   - ✅ Both binaries execute successfully

---

## 🐛 Potential Issues

### Issue 1: Cargo Incremental Compilation

**Problem**: Cargo might not detect source changes if:
- File modification times aren't updating correctly
- Incremental compilation cache is stale
- Build artifacts are cached incorrectly

**Symptoms**:
- `cargo build --release` shows "Fresh" for main binary when it should rebuild
- Source changes don't trigger recompilation

**Diagnosis**:
```bash
# Check if Cargo detects changes
cd ~/nabia/core/nabi-cli
touch src/main.rs
cargo build --release --verbose 2>&1 | grep -E "(Compiling|Fresh)" | head -5

# Check incremental cache
ls -la ~/.cache/nabi/nabi-cli/target/release/incremental/
```

**Solution**:
```bash
# Force clean rebuild
cargo clean
cargo build --release

# Or disable incremental compilation for release builds
# Add to Cargo.toml:
[profile.release]
incremental = false
```

### Issue 2: Symlink Copy Behavior

**Problem**: When copying to a symlink destination:
- `cp source symlink` follows the symlink and overwrites the target
- This should work, but timing/permissions might cause issues
- Code signing happens after copy, which changes the binary

**Symptoms**:
- Binary copied but not updated
- Timestamp mismatch between build and install

**Diagnosis**:
```bash
# Check symlink behavior
cd ~/nabia/core/nabi-cli
ls -la ~/.local/share/nabi/bin/nabi
readlink ~/.local/share/nabi/bin/nabi

# Test copy behavior
cp ~/.cache/nabi/nabi-cli/target/release/nabi ~/.local/share/nabi/bin/nabi
ls -lh ~/.local/bin/nabi  # Should show updated timestamp
```

**Solution**:
```bash
# Option 1: Remove symlink before copy (in justfile)
rm -f "${NABI_BIN}/nabi"
cp "${BINARY}" "${NABI_BIN}/nabi"

# Option 2: Copy directly to target (bypass symlink)
cp "${BINARY}" "$(readlink -f "${NABI_BIN}/nabi")"
```

### Issue 3: Binary Caching

**Problem**: macOS or system might cache the binary:
- Code signing might cache the binary
- System integrity protection might prevent updates
- File system caching

**Symptoms**:
- Binary appears updated but old code runs
- `otool` or `strings` shows old code in binary

**Diagnosis**:
```bash
# Check binary contents
strings ~/.local/bin/nabi | grep -i "version\|test_marker"

# Check code signature
codesign -dv ~/.local/bin/nabi
codesign -dvvv ~/.local/bin/nabi 2>&1 | grep -E "(Authority|Timestamp)"

# Compare with build binary
diff <(strings ~/.cache/nabi/nabi-cli/target/release/nabi) <(strings ~/.local/bin/nabi) | head -20
```

**Solution**:
```bash
# Remove code signature before copy, re-sign after
codesign --remove-signature ~/.local/bin/nabi  # If needed
# Then re-sign after copy
```

---

## 🛠️ Diagnostic Tools

### 1. Deployment Diagnostic Script

**Location**: `scripts/debug-deploy.sh`

**Usage**:
```bash
cd ~/nabia/core/nabi-cli
./scripts/debug-deploy.sh
```

**Checks**:
- Build artifact existence and timestamps
- Symlink chain integrity
- PATH resolution
- MD5 comparison (before signing)
- Cargo cache status

### 2. Rebuild Test Script

**Location**: `scripts/test-rebuild.sh`

**Usage**:
```bash
cd ~/nabia/core/nabi-cli
./scripts/test-rebuild.sh
```

**Tests**:
- Makes a test change to source
- Rebuilds and verifies change is in binary
- Restores original state

---

## 🔧 Recommended Fixes

### Fix 1: Improve Installation Process

**Update `justfile` install target**:

```bash
@install: completions sign
    #!/bin/bash
    set -e
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    NABI_BIN="${XDG_DATA}/nabi/bin"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"

    # Resolve symlink target
    if [ -L "${NABI_BIN}/nabi" ]; then
        TARGET=$(readlink -f "${NABI_BIN}/nabi" || readlink "${NABI_BIN}/nabi")
        echo "📦 Installing to symlink target: ${TARGET}"
        mkdir -p "$(dirname "${TARGET}")"
        cp "${BINARY}" "${TARGET}"
        chmod +x "${TARGET}"
        INSTALL_PATH="${TARGET}"
    else
        mkdir -p "${NABI_BIN}"
        cp "${BINARY}" "${NABI_BIN}/nabi"
        chmod +x "${NABI_BIN}/nabi"
        INSTALL_PATH="${NABI_BIN}/nabi"
    fi

    # Re-sign the installed binary
    # ... (existing signing code)

    # Verify installation
    if "${INSTALL_PATH}" --version >/dev/null 2>&1; then
        echo "✅ Installed nabi to ${INSTALL_PATH}"
    else
        echo "❌ Installation verification failed"
        exit 1
    fi
```

### Fix 2: Force Rebuild Detection

**Add to `justfile`**:

```bash
# Force rebuild (clean + build)
@rebuild: clean build
    echo "✅ Clean rebuild complete"

# Verify source changes trigger rebuild
@verify-rebuild:
    #!/bin/bash
    touch src/main.rs
    cargo build --release --verbose 2>&1 | grep -E "(Compiling|Fresh)" | head -5
```

### Fix 3: Add Build Verification

**Add to `justfile`**:

```bash
# Verify build matches source
@verify-build:
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"

    if [ ! -f "${BINARY}" ]; then
        echo "❌ Build binary not found"
        exit 1
    fi

    # Check if binary is newer than source
    SOURCE_NEWEST=$(find src/ -type f -exec stat -f "%m" {} \; | sort -n | tail -1)
    BINARY_TIME=$(stat -f "%m" "${BINARY}")

    if [ "${BINARY_TIME}" -lt "${SOURCE_NEWEST}" ]; then
        echo "⚠️  Binary is older than source files"
        echo "   Run: just rebuild"
        exit 1
    else
        echo "✅ Binary is up-to-date"
    fi
```

---

## 📋 Testing Checklist

When investigating deployment issues:

- [ ] Run `./scripts/debug-deploy.sh` to get baseline
- [ ] Make a test change to source code
- [ ] Run `cargo build --release --verbose` and verify "Compiling nabi" appears
- [ ] Check build binary timestamp: `ls -lh ~/.cache/nabi/nabi-cli/target/release/nabi`
- [ ] Run `just install` and verify copy succeeds
- [ ] Check installed binary timestamp: `ls -lh ~/.local/bin/nabi`
- [ ] Verify binary executes: `nabi --version`
- [ ] Check if change is in binary: `strings ~/.local/bin/nabi | grep "test_change"`
- [ ] Run `./scripts/test-rebuild.sh` for automated test

---

## 🎯 Next Steps

1. **Run diagnostic**: `./scripts/debug-deploy.sh`
2. **Test rebuild**: `./scripts/test-rebuild.sh`
3. **If rebuild fails**: Check Cargo incremental compilation
4. **If install fails**: Check symlink copy behavior
5. **If binary stale**: Check code signing and caching

---

## 📚 Related Files

- `justfile` - Build and installation recipes
- `.cargo/config.toml` - Cargo configuration (XDG paths)
- `scripts/debug-deploy.sh` - Diagnostic tool
- `scripts/test-rebuild.sh` - Rebuild verification tool

---

## 🔗 References

- [Cargo Incremental Compilation](https://doc.rust-lang.org/cargo/reference/profiles.html#incremental)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
- [macOS Code Signing](https://developer.apple.com/documentation/security/code_signing_services)
