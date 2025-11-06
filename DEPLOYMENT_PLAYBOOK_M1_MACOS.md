# NabiOS Deployment Playbook: M1 MacBook Pro (macOS Sonoma/Ventura)

**Target Platform**: M1/M2/M3 MacBook Pro (ARM64 architecture)
**OS Versions**: macOS Sonoma 14.x, Ventura 13.x
**Total Time**: 45-60 minutes (clean installation)
**Status**: Production Ready
**Last Updated**: 2025-10-30

---

## Executive Summary

This playbook deploys the **nabi-cli federation gateway** on a fresh M1 MacBook Pro, establishing:
- ✅ Rust-based CLI router (Layer 1)
- ✅ Bash routing layer (Layer 2)
- ✅ Python tool ecosystem (Layer 3)
- ✅ XDG-compliant directory structure
- ⏳ Basic federation capabilities (coordination, memory)

**What Works After Installation**:
- `nabi self doctor` - System health checks
- `nabi self config` - Configuration display
- `nabi repo check` - Repository validation
- `nabi --version` - Version information
- Python CLI tools via `nabi-python` shim

**Known Limitations**:
- SurrealDB migration incomplete (61% - 496/804 entities)
- Federation coordination unverified on fresh installs
- Memory system falls back to file-based (memory.json)
- Commander plugins not yet implemented (future roadmap)

---

## Prerequisites Check

**Run these commands BEFORE starting installation**:

```bash
# 1. Check macOS version (must be 13.x or 14.x)
sw_vers

# Expected output:
# ProductName:        macOS
# ProductVersion:     14.x.x or 13.x.x
# BuildVersion:       ...

# 2. Check CPU architecture (must be ARM64)
uname -m

# Expected output: arm64

# 3. Check available disk space (need at least 5GB)
df -h ~

# Expected output: At least 5GB available

# 4. Check if Homebrew is installed
which brew

# If not found, install Homebrew first:
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

**Estimated Time**: 5 minutes

---

## Phase 1: System Dependencies (15 minutes)

### Step 1.1: Install Rust Toolchain for ARM64

```bash
# Install rustup (Rust toolchain installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Choose option 1 (default installation)
# This installs:
# - rustc (Rust compiler)
# - cargo (package manager)
# - rustup (toolchain manager)
```

**Expected Output**:
```
Rust is installed now. Great!

To configure your current shell, you need to source
the corresponding env file under $HOME/.cargo/env.
```

**Verify Installation**:
```bash
# Source the environment
source "$HOME/.cargo/env"

# Check Rust version (should be 1.70+)
rustc --version

# Expected: rustc 1.88.0 (or newer)

# Check cargo version
cargo --version

# Expected: cargo 1.88.0 (or newer)

# Verify ARM64 target
rustc --version --verbose | grep host

# Expected: host: aarch64-apple-darwin
```

**Estimated Time**: 5 minutes

### Step 1.2: Install Python with uv

```bash
# Install uv (modern Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Verify installation
uv --version

# Expected: uv 0.8.22 (or newer)

# Install Python 3.13 (recommended)
uv python install 3.13

# Verify Python installation
uv python list

# Expected output should include: python3.13
```

**Estimated Time**: 5 minutes

### Step 1.3: Set Up XDG Environment Variables

```bash
# Add to ~/.zshenv (macOS default shell)
cat >> ~/.zshenv << 'EOF'

# XDG Base Directory Specification (forced on macOS for compatibility)
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

# Add ~/.local/bin to PATH for nabi binary
export PATH="$HOME/.local/bin:$PATH"

EOF

# Apply changes
source ~/.zshenv

# Verify XDG variables
echo "XDG_CONFIG_HOME: $XDG_CONFIG_HOME"
echo "XDG_CACHE_HOME: $XDG_CACHE_HOME"
echo "XDG_DATA_HOME: $XDG_DATA_HOME"
echo "XDG_STATE_HOME: $XDG_STATE_HOME"

# Expected output:
# XDG_CONFIG_HOME: /Users/<username>/.config
# XDG_CACHE_HOME: /Users/<username>/.cache
# XDG_DATA_HOME: /Users/<username>/.local/share
# XDG_STATE_HOME: /Users/<username>/.local/state
```

**Estimated Time**: 2 minutes

### Step 1.4: Create XDG Directory Structure

```bash
# Create all required XDG directories
mkdir -p ~/.config/nabi
mkdir -p ~/.cache/nabi
mkdir -p ~/.local/share/nabi
mkdir -p ~/.local/state/nabi
mkdir -p ~/.local/bin

# Create development directories (optional, for source code)
mkdir -p ~/nabia/core
mkdir -p ~/nabia/tools
mkdir -p ~/nabia/platform

# Verify directory structure
ls -la ~/.config/nabi
ls -la ~/.cache/nabi
ls -la ~/.local/share/nabi
ls -la ~/.local/state/nabi
ls -la ~/.local/bin

# All should exist and be empty (no errors)
```

**Estimated Time**: 1 minute

---

## Phase 2: Install nabi-cli Binary (20 minutes)

### Step 2.1: Clone nabi-cli Source

```bash
# Clone to development directory
cd ~/nabia/core
git clone https://github.com/nabia-federation/nabi-cli.git

# If repository is private or doesn't exist yet, you may need to:
# 1. Request access
# 2. Copy from existing installation
# 3. Use local tarball

# For now, assuming source is at: ~/nabia/core/nabi-cli/
cd ~/nabia/core/nabi-cli
```

**Alternate Method (if no git repository)**:
```bash
# If you have an existing nabi-cli directory, copy it:
cp -r /path/to/existing/nabi-cli ~/nabia/core/nabi-cli
cd ~/nabia/core/nabi-cli
```

**Estimated Time**: 2 minutes

### Step 2.2: Generate Cargo Configuration

```bash
# Generate XDG-compliant cargo config
cd ~/nabia/core/nabi-cli
make config

# This creates .cargo/config.toml with XDG paths
# Verify it was created:
cat .cargo/config.toml

# Expected output:
# [build]
# target-dir = "/Users/<username>/.cache/nabi/nabi-cli/target"
```

**Troubleshooting**:
If `make config` fails:
```bash
# Manually create the config
mkdir -p .cargo
sed "s|\$XDG_CACHE_HOME|$HOME/.cache|g" .cargo/config.toml.template > .cargo/config.toml
```

**Estimated Time**: 1 minute

### Step 2.3: Build nabi Binary (ARM64)

```bash
# Build release binary (optimized for M1)
cd ~/nabia/core/nabi-cli
make build

# This will:
# 1. Download and compile dependencies (first time: 10-15 minutes)
# 2. Compile nabi source code
# 3. Output to: ~/.cache/nabi/nabi-cli/target/release/nabi

# You'll see output like:
#    Compiling nabi v0.1.0 (/Users/<username>/nabia/core/nabi-cli)
#    Finished release [optimized] target(s) in 8m 32s
```

**Expected Build Time**:
- First build: 10-15 minutes (downloads dependencies)
- Subsequent builds: 30-60 seconds (incremental)

**Troubleshooting**:
If build fails with "could not compile":
```bash
# Check Rust version (must be 1.70+)
rustc --version

# Check cargo is working
cargo --version

# Try cleaning and rebuilding
make clean
make build
```

**Estimated Time**: 15 minutes (first build)

### Step 2.4: Install nabi Binary

```bash
# Install to ~/.local/bin
cd ~/nabia/core/nabi-cli
make install

# This copies:
# ~/.cache/nabi/nabi-cli/target/release/nabi → ~/.local/bin/nabi

# Verify installation
ls -lh ~/.local/bin/nabi

# Expected output:
# -rwxr-xr-x  1 <user>  staff   2.0M Oct 30 22:47 /Users/<username>/.local/bin/nabi

# Test binary
which nabi

# Expected: /Users/<username>/.local/bin/nabi

# Test version
nabi --version

# Expected: nabi 0.1.0
```

**Troubleshooting**:
If `which nabi` doesn't find it:
```bash
# Check PATH includes ~/.local/bin
echo $PATH | grep ".local/bin"

# If not found, add to ~/.zshenv:
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshenv
source ~/.zshenv
```

**Estimated Time**: 2 minutes

---

## Phase 3: Python CLI Layer (10 minutes)

### Step 3.1: Install nabi-python Shim

```bash
# Create tools directory if it doesn't exist
mkdir -p ~/nabia/tools

# Copy or symlink nabi-python shim
# Option A: If you have existing nabi-python:
cp /path/to/existing/nabi-python ~/nabia/tools/nabi-python
chmod +x ~/nabia/tools/nabi-python

# Option B: Create symlink in ~/.local/bin
ln -sf ~/nabia/tools/nabi-python ~/.local/bin/nabi-python

# Verify installation
ls -la ~/.local/bin/nabi-python

# Expected: symlink to ~/nabia/tools/nabi-python

# Test shim
nabi-python --help

# Expected: Usage information or command list
```

**Note**: The nabi-python shim is a bash script that routes commands to Python implementations. It's part of Layer 2 routing.

**Estimated Time**: 2 minutes

### Step 3.2: Create Python Virtual Environments Directory

```bash
# Create venvs directory for Python tools
mkdir -p ~/.local/share/nabi/venvs

# Create convenience symlink
ln -sf ~/.local/share/nabi/venvs ~/.nabi/venvs

# Verify structure
ls -la ~/.local/share/nabi/venvs
ls -la ~/.nabi/venvs
```

**Estimated Time**: 1 minute

### Step 3.3: Install Basic Python Utilities

```bash
# Example: Install doctor utility (if available)
# This would typically be done via:
# uv venv ~/.local/share/nabi/venvs/nabi-utils
# uv pip install --venv ~/.local/share/nabi/venvs/nabi-utils <packages>

# For now, just create the structure
mkdir -p ~/.config/nabi/lib

# Copy Python utilities if available
# (doctor.py, syncthing-status.py, etc.)
```

**Estimated Time**: 2 minutes

---

## Phase 4: Verification & Testing (10 minutes)

### Step 4.1: Run Health Check

```bash
# Run comprehensive health check
nabi self doctor

# Expected output:
# 🏥 Running health check...
#   ✓ XDG directories present
#   ✓ Binary installed correctly
#   ✓ Configuration valid
#   (various checks...)
# ✓ Health check complete!
```

**Troubleshooting**:
If health check fails:
```bash
# Check what's wrong
nabi self config

# This shows current configuration and paths
# Look for any errors or missing directories
```

**Estimated Time**: 2 minutes

### Step 4.2: Test Core Commands

```bash
# Test version
nabi --version
# Expected: nabi 0.1.0

# Test help
nabi --help
# Expected: Usage information with all subcommands

# Test config display
nabi self config
# Expected: Configuration dump with paths

# Test repo check (if in a git repo)
cd ~/nabia/core/nabi-cli
nabi repo check
# Expected: Repository compliance report
```

**Estimated Time**: 3 minutes

### Step 4.3: Verify XDG Compliance

```bash
# Check all XDG paths are being used
nabi self config | grep -E "(CONFIG|CACHE|DATA|STATE)_HOME"

# Expected output should show:
# XDG_CONFIG_HOME: /Users/<username>/.config
# XDG_CACHE_HOME: /Users/<username>/.cache
# XDG_DATA_HOME: /Users/<username>/.local/share
# XDG_STATE_HOME: /Users/<username>/.local/state

# Verify build artifacts are in cache
ls -la ~/.cache/nabi/nabi-cli/target/

# Expected: Cargo build artifacts (target/ directory)

# Verify no artifacts in source directory
ls -la ~/nabia/core/nabi-cli/ | grep target

# Expected: Only .cargo/config.toml (no target/ directory)
```

**Estimated Time**: 2 minutes

### Step 4.4: Test Python Layer (if installed)

```bash
# Test nabi-python shim
nabi-python --help

# If riff-cli or other tools are installed:
# nabi-python riff --version

# Test Layer 2 routing
# (This depends on what commanders are installed)
```

**Estimated Time**: 3 minutes

---

## Phase 5: Post-Installation Setup (Optional)

### Step 5.1: Create Unified Access Layer

```bash
# Create ~/.nabi/ convenience symlinks
mkdir -p ~/.nabi

# Symlink to XDG directories
ln -sf ~/.local/state/nabi ~/.nabi/state
ln -sf ~/.local/share/nabi ~/.nabi/share
ln -sf ~/.cache/nabi ~/.nabi/cache
ln -sf ~/.config/nabi ~/.nabi/config

# Symlink to development directories
ln -sf ~/nabia/core ~/.nabi/source
ln -sf ~/nabia/platform ~/.nabi/platform
ln -sf ~/nabia/tools ~/.nabi/tools

# Symlink to bin
ln -sf ~/.local/share/nabi/bin ~/.nabi/bin

# Verify unified access
ls -la ~/.nabi/

# Expected: Symlinks to all major directories
```

**Estimated Time**: 2 minutes

### Step 5.2: Enable Shell Completions (Optional)

```bash
# Generate shell completions for zsh
nabi completions zsh > ~/.local/share/zsh/site-functions/_nabi

# Or add to .zshrc:
echo 'eval "$(nabi completions zsh)"' >> ~/.zshrc
source ~/.zshrc

# Test tab completion
nabi self <TAB>
# Expected: Shows available subcommands (doctor, config, etc.)
```

**Estimated Time**: 2 minutes

---

## Success Criteria

### ✅ Minimum Viable Installation

After completing Phases 1-4, you should have:

1. **Rust toolchain installed**
   ```bash
   rustc --version  # Shows 1.70+
   cargo --version  # Shows 1.70+
   ```

2. **nabi binary installed and working**
   ```bash
   which nabi  # Shows ~/.local/bin/nabi
   nabi --version  # Shows version 0.1.0
   nabi self doctor  # Passes health checks
   ```

3. **XDG directories created**
   ```bash
   ls -ld ~/.config/nabi  # Exists
   ls -ld ~/.cache/nabi   # Exists
   ls -ld ~/.local/share/nabi  # Exists
   ls -ld ~/.local/state/nabi  # Exists
   ```

4. **Python environment ready**
   ```bash
   uv --version  # Shows uv version
   which nabi-python  # Shows symlink (if installed)
   ```

### ✅ Full Installation (with Python layer)

Additionally:

5. **Python CLI layer working**
   ```bash
   nabi-python --help  # Shows usage
   ls -la ~/.local/share/nabi/venvs/  # Shows venv structure
   ```

6. **Unified access layer created**
   ```bash
   ls -la ~/.nabi/  # Shows symlinks
   ```

---

## Known Limitations & Workarounds

### 1. SurrealDB Migration Incomplete

**Status**: 61% complete (496/804 entities migrated)

**Impact**:
- Memory system falls back to file-based (memory.json)
- Graph queries work but with reduced dataset
- Federation events may not be fully tracked

**Workaround**:
```bash
# Memory system automatically falls back to JSON
# No action required, but expect limited memory functionality
```

### 2. Commander Plugins Not Implemented

**Status**: Foundation complete, commanders pending

**Impact**:
- `nabi claude` shows "commander not implemented"
- `nabi data` shows "commander not implemented"
- `nabi federation` shows "commander not implemented"

**Workaround**:
```bash
# Use nabi-python shim for Python-based commands
nabi-python claude session list

# Or use direct tools
cd ~/nabia/tools/claude-manager
./claude-manager.sh list
```

### 3. Federation Coordination Unverified

**Status**: Architecture in place, needs testing on fresh install

**Impact**:
- Agent coordination may need manual setup
- Federation events may not propagate
- Memory synchronization uncertain

**Workaround**:
```bash
# Verify federation state directory exists
mkdir -p ~/.local/state/nabi/federation

# Check for federation registry
ls -la ~/.local/state/nabi/federation-registry.json

# If missing, federation features won't work yet
```

---

## Troubleshooting Guide

### Issue: `nabi: command not found`

**Cause**: PATH doesn't include ~/.local/bin

**Fix**:
```bash
# Add to ~/.zshenv
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshenv
source ~/.zshenv

# Verify
which nabi
```

### Issue: Build fails with "linker not found"

**Cause**: Xcode Command Line Tools not installed

**Fix**:
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Retry build
cd ~/nabia/core/nabi-cli
make clean
make build
```

### Issue: `rustc: command not found`

**Cause**: Rust environment not sourced

**Fix**:
```bash
# Source Rust environment
source "$HOME/.cargo/env"

# Add to shell profile
echo 'source "$HOME/.cargo/env"' >> ~/.zshenv
```

### Issue: Permission denied on ~/.local/bin/nabi

**Cause**: Binary not executable

**Fix**:
```bash
chmod +x ~/.local/bin/nabi
```

### Issue: Build artifacts fill up disk space

**Cause**: Cargo cache growing too large

**Fix**:
```bash
# Clean old build artifacts
cd ~/nabia/core/nabi-cli
make clean

# Or clean all cargo cache
cargo clean
rm -rf ~/.cache/nabi/nabi-cli/target
```

---

## Rollback Procedure

If installation fails and you need to start fresh:

```bash
# Stop here and backup if you have important data
# This will DELETE all nabi-related directories

# Remove XDG directories
rm -rf ~/.config/nabi
rm -rf ~/.cache/nabi
rm -rf ~/.local/share/nabi
rm -rf ~/.local/state/nabi

# Remove binaries
rm -f ~/.local/bin/nabi
rm -f ~/.local/bin/nabi-python

# Remove development directories (optional)
rm -rf ~/nabia/core/nabi-cli
rm -rf ~/nabia/tools/nabi-python

# Remove unified access layer
rm -rf ~/.nabi

# Remove environment variables from ~/.zshenv
# (manually edit and remove XDG_* exports)

# Restart shell
exec zsh
```

---

## Next Steps

After successful installation:

1. **Install Additional Tools**
   - riff-cli (search and archive tool)
   - claude-manager (session management)
   - Other federation tools

2. **Configure Federation**
   - Set up agent coordination
   - Configure memory synchronization
   - Enable federation events

3. **Customize Environment**
   - Add shell aliases
   - Configure auras (if using)
   - Set up hooks (if needed)

4. **Join Federation Network**
   - Connect to Raspberry Pi coordination server
   - Set up Syncthing for file sync
   - Configure Loki for event monitoring

---

## Support & Resources

**Documentation**:
- Architecture: `~/docs/architecture/UNIFIED_CONVERGENCE_PLAN.md`
- CLI Reference: `~/docs/tools/nabi-cli.md`
- XDG Topology: `~/docs/architecture/XDG_TOPOLOGY_SINGLE_SPAN.md`

**Commands for Help**:
```bash
nabi --help              # General help
nabi self doctor --help  # Doctor command help
nabi repo check --help   # Repo check help
```

**Common Issues**:
- Check GitHub issues: (if repository is public)
- Check federation docs: `~/docs/federation/`

---

**Deployment Status**: ✅ Ready for Production
**Platform**: M1/M2/M3 MacBook Pro (ARM64)
**Installation Time**: 45-60 minutes
**Difficulty**: Intermediate (requires command line experience)
