# NabiOS Deployment Troubleshooting Guide

**Purpose**: Comprehensive troubleshooting for common deployment issues
**Platforms**: M1 macOS and WSL2 Ubuntu
**Last Updated**: 2025-10-30

---

## Table of Contents

1. [Pre-Installation Issues](#pre-installation-issues)
2. [Dependency Installation Issues](#dependency-installation-issues)
3. [Build Issues](#build-issues)
4. [Installation Issues](#installation-issues)
5. [Runtime Issues](#runtime-issues)
6. [Platform-Specific Issues](#platform-specific-issues)
7. [Performance Issues](#performance-issues)
8. [Known Limitations](#known-limitations)
9. [Recovery Procedures](#recovery-procedures)

---

## Pre-Installation Issues

### Issue: Insufficient Disk Space

**Symptoms**:
```bash
df -h ~
# Shows less than 5GB available
```

**Impact**: Build will fail or system will become unstable

**Fix**:
```bash
# M1 macOS - Clear caches
rm -rf ~/Library/Caches/*
brew cleanup  # If using Homebrew

# WSL2 Ubuntu - Clear apt cache
sudo apt clean
sudo apt autoclean

# Both - Clear old Rust build artifacts (if any)
cargo clean  # In any old projects
rm -rf ~/.cargo/registry/cache/*

# Check space again
df -h ~
```

**Prevention**: Monitor disk usage regularly
```bash
# Add to ~/.zshrc or ~/.bashrc
alias disk-check='df -h ~ | grep -E "Use%|/$"'
```

---

### Issue: Wrong Architecture Detected

**Symptoms (M1 macOS)**:
```bash
uname -m
# Shows: x86_64 (instead of arm64)
```

**Cause**: Running terminal under Rosetta 2 emulation

**Fix**:
```bash
# Check if running under Rosetta
sysctl sysctl.proc_translated
# Should return: 0 (native) not 1 (translated)

# If translated (1), you need to:
# 1. Quit Terminal/iTerm
# 2. Right-click Terminal.app → Get Info
# 3. Uncheck "Open using Rosetta"
# 4. Restart Terminal

# Verify
uname -m
# Should now show: arm64
```

**Symptoms (WSL2 Ubuntu)**:
```bash
uname -m
# Shows something other than x86_64
```

**Cause**: Wrong WSL distribution installed

**Fix**:
```powershell
# From PowerShell
wsl --list --verbose
# Check you're using Ubuntu-22.04 x86_64

# If wrong architecture, reinstall:
wsl --unregister Ubuntu-22.04
wsl --install -d Ubuntu-22.04
```

---

### Issue: macOS Version Too Old

**Symptoms**:
```bash
sw_vers
# ProductVersion: 12.x or earlier
```

**Cause**: macOS Monterey (12.x) or older

**Impact**: Some Rust features may not work correctly

**Fix**:
- Upgrade to macOS Ventura (13.x) or Sonoma (14.x)
- Or use older Rust toolchain (not recommended)

**Workaround** (if upgrade not possible):
```bash
# Install specific Rust version
rustup install 1.70.0
rustup default 1.70.0
```

---

## Dependency Installation Issues

### Issue: Rust Installation Fails

**Symptoms**:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Error: curl: (60) SSL certificate problem
```

**Cause**: System certificates outdated or network issues

**Fix (M1 macOS)**:
```bash
# Update system certificates
sudo softwareupdate --install --all

# Or install via Homebrew
brew install rustup
rustup-init
```

**Fix (WSL2 Ubuntu)**:
```bash
# Update certificates
sudo apt update
sudo apt install -y ca-certificates

# Retry Rust installation
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

### Issue: `rustc: command not found` After Installation

**Symptoms**:
```bash
rustc --version
# bash: rustc: command not found
```

**Cause**: Environment not sourced or PATH not set

**Fix (M1 macOS)**:
```bash
# Source immediately
source "$HOME/.cargo/env"

# Verify
rustc --version

# Make permanent
cat >> ~/.zshenv << 'EOF'
# Rust environment
source "$HOME/.cargo/env"
EOF

# Reload shell
exec zsh
```

**Fix (WSL2 Ubuntu)**:
```bash
# Source immediately
source "$HOME/.cargo/env"

# Verify
rustc --version

# Make permanent
cat >> ~/.bashrc << 'EOF'
# Rust environment
source "$HOME/.cargo/env"
EOF

# Reload shell
exec bash
```

---

### Issue: uv Installation Fails

**Symptoms**:
```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
# Error: Permission denied
```

**Cause**: Installation script needs write access to ~/.cargo/bin

**Fix**:
```bash
# Ensure ~/.cargo/bin exists and is writable
mkdir -p ~/.cargo/bin
chmod 755 ~/.cargo/bin

# Retry installation
curl -LsSf https://astral.sh/uv/install.sh | sh

# Source environment
source "$HOME/.cargo/env"

# Verify
uv --version
```

**Alternative** (manual installation):
```bash
# Download directly
cargo install uv

# Or use pip (not recommended)
pip install uv
```

---

### Issue: Xcode Command Line Tools Not Installing (macOS)

**Symptoms**:
```bash
xcode-select --install
# Error: Can't install the software because it is not currently available
```

**Cause**: Tools already installed or macOS version issue

**Fix**:
```bash
# Check if already installed
xcode-select -p
# If shows path, tools are installed

# If not installed, try manual download:
# 1. Visit https://developer.apple.com/download/more/
# 2. Search for "Command Line Tools for Xcode"
# 3. Download version matching your macOS
# 4. Install .dmg file

# Verify
xcode-select -p
# Should show: /Library/Developer/CommandLineTools
```

---

## Build Issues

### Issue: Build Fails with "linker `cc` not found"

**Symptoms**:
```bash
make build
# error: linker `cc` not found
```

**Cause**: C compiler not installed

**Fix (M1 macOS)**:
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Verify
cc --version
# Should show: Apple clang version ...

# Retry build
cd ~/nabia/core/nabi-cli
make clean
make build
```

**Fix (WSL2 Ubuntu)**:
```bash
# Install build-essential
sudo apt update
sudo apt install -y build-essential

# Verify
cc --version
# Should show: gcc ...

# Retry build
cd ~/nabia/core/nabi-cli
make clean
make build
```

---

### Issue: Build Fails with "could not compile" Errors

**Symptoms**:
```bash
make build
# error: could not compile `nabi` due to 3 previous errors
```

**Causes**: Various (outdated Rust, corrupted dependencies, etc.)

**Diagnostic Steps**:
```bash
# 1. Check Rust version
rustc --version
# Must be 1.70+

# 2. Check cargo version
cargo --version

# 3. Check target architecture
rustc --version --verbose | grep host

# 4. Clean all artifacts
cargo clean
rm -rf ~/.cache/nabi/nabi-cli/target/

# 5. Update Rust
rustup update

# 6. Retry build with verbose output
cd ~/nabia/core/nabi-cli
cargo build --release --verbose
```

**Common Fixes**:
```bash
# Fix 1: Update dependencies
cd ~/nabia/core/nabi-cli
cargo update

# Fix 2: Clear cargo registry cache
rm -rf ~/.cargo/registry/index/*
rm -rf ~/.cargo/registry/cache/*

# Fix 3: Reinstall Rust
rustup self uninstall
# Then reinstall from scratch
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

### Issue: Build Extremely Slow (30+ minutes)

**Symptoms**:
```bash
make build
# Takes 30+ minutes (should be 10-15 minutes max)
```

**Cause (M1 macOS)**: Running under Rosetta 2 emulation

**Fix**:
```bash
# Check if emulated
sysctl sysctl.proc_translated
# If returns 1, you're emulated

# Fix Terminal Rosetta setting (see Pre-Installation Issues)
# Then retry build
```

**Cause (WSL2 Ubuntu)**: Building on Windows filesystem

**Fix**:
```bash
# Check current directory
pwd
# If shows /mnt/c/... you're on Windows filesystem

# Move to Linux filesystem
cd ~
mv /mnt/c/projects/nabi-cli ~/nabia/core/nabi-cli
cd ~/nabia/core/nabi-cli

# Retry build
make clean
make build
```

**Cause (Both)**: Insufficient CPU/RAM

**Fix**:
```bash
# Check system resources
top  # On macOS/Linux

# Close other applications
# Allocate more resources (WSL2 only - see .wslconfig)
```

---

### Issue: Build Artifacts Fill Up Disk

**Symptoms**:
```bash
du -sh ~/.cache/nabi/nabi-cli/target/
# Shows 5GB+ (should be ~1-2GB)
```

**Cause**: Incremental builds accumulate artifacts

**Fix**:
```bash
# Clean build artifacts
cd ~/nabia/core/nabi-cli
make clean

# Or clean all cargo cache
cargo clean

# Verify
du -sh ~/.cache/nabi/nabi-cli/target/
```

**Prevention**:
```bash
# Periodic cleanup script
cat > ~/cleanup-nabi-build.sh << 'EOF'
#!/bin/bash
echo "Cleaning nabi build artifacts..."
cd ~/nabia/core/nabi-cli
cargo clean
echo "Done. Freed:"
du -sh ~/.cache/nabi/nabi-cli/target/ 2>/dev/null || echo "Cache already clean"
EOF

chmod +x ~/cleanup-nabi-build.sh

# Run weekly or as needed
~/cleanup-nabi-build.sh
```

---

### Issue: make config Fails

**Symptoms**:
```bash
make config
# make: .cargo/config.toml.template: No such file or directory
```

**Cause**: Missing template file or wrong directory

**Fix**:
```bash
# Verify you're in correct directory
cd ~/nabia/core/nabi-cli
ls .cargo/config.toml.template
# Should exist

# If missing, check git status
git status

# If file truly missing, create manually:
mkdir -p .cargo
cat > .cargo/config.toml << EOF
[build]
target-dir = "$HOME/.cache/nabi/nabi-cli/target"
EOF
```

---

## Installation Issues

### Issue: make install Fails with Permission Denied

**Symptoms**:
```bash
make install
# cp: cannot create regular file '~/.local/bin/nabi': Permission denied
```

**Cause**: ~/.local/bin doesn't exist or isn't writable

**Fix**:
```bash
# Create directory
mkdir -p ~/.local/bin

# Ensure writable
chmod 755 ~/.local/bin

# Retry install
make install
```

---

### Issue: Binary Not Found After Installation

**Symptoms**:
```bash
which nabi
# (no output)

nabi --version
# bash: nabi: command not found
```

**Cause**: ~/.local/bin not in PATH

**Fix (M1 macOS)**:
```bash
# Check PATH
echo $PATH | grep ".local/bin"
# If no output, PATH is wrong

# Add to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshenv
source ~/.zshenv

# Verify
which nabi
# Should show: /Users/<username>/.local/bin/nabi
```

**Fix (WSL2 Ubuntu)**:
```bash
# Check PATH
echo $PATH | grep ".local/bin"

# Add to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify
which nabi
```

---

### Issue: Binary Exists But Won't Execute

**Symptoms**:
```bash
ls -la ~/.local/bin/nabi
# -rw-r--r-- ... (note: not executable)

nabi --version
# bash: nabi: Permission denied
```

**Cause**: Binary not marked executable

**Fix**:
```bash
# Make executable
chmod +x ~/.local/bin/nabi

# Verify
ls -la ~/.local/bin/nabi
# Should show: -rwxr-xr-x

# Test
nabi --version
```

---

## Runtime Issues

### Issue: nabi self doctor Fails

**Symptoms**:
```bash
nabi self doctor
# ❌ XDG directories missing
# ❌ Configuration invalid
```

**Fix**:
```bash
# Create missing XDG directories
mkdir -p ~/.config/nabi
mkdir -p ~/.cache/nabi
mkdir -p ~/.local/share/nabi
mkdir -p ~/.local/state/nabi

# Verify environment variables
echo $XDG_CONFIG_HOME
echo $XDG_CACHE_HOME
echo $XDG_DATA_HOME
echo $XDG_STATE_HOME

# If empty, source environment
# M1 macOS:
source ~/.zshenv

# WSL2 Ubuntu:
source ~/.bashrc

# Retry
nabi self doctor
```

---

### Issue: nabi Commands Show "Not Implemented"

**Symptoms**:
```bash
nabi claude session list
# ℹ️  Commander implementation pending
```

**Cause**: This is **EXPECTED** - commander plugins not yet implemented

**Status**: Foundation phase complete, commanders are future roadmap

**Workaround**:
```bash
# Use nabi-python shim for Python-based commands
nabi-python claude session list

# Or use tools directly
~/nabia/tools/claude-manager/claude-manager.sh list
```

**When will this work?**
- Commanders are planned for future implementation
- See `CLAUDE.md` for roadmap
- Current focus: Foundation layer (working)

---

### Issue: XDG Path Variables Not Set

**Symptoms**:
```bash
echo $XDG_CONFIG_HOME
# (empty)

nabi self config
# Shows wrong paths or errors
```

**Cause**: Environment not sourced

**Fix (M1 macOS)**:
```bash
# Check ~/.zshenv exists
cat ~/.zshenv | grep XDG

# If missing, recreate:
cat >> ~/.zshenv << 'EOF'
# XDG Base Directory Specification
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
EOF

# Reload
source ~/.zshenv

# Verify
echo $XDG_CONFIG_HOME
```

**Fix (WSL2 Ubuntu)**:
```bash
# Same process, but use ~/.bashrc instead
cat >> ~/.bashrc << 'EOF'
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
EOF

source ~/.bashrc
```

---

## Platform-Specific Issues

### M1 macOS: Homebrew Conflicts

**Symptoms**:
```bash
which rustc
# /opt/homebrew/bin/rustc (not ~/.cargo/bin/rustc)

rustc --version
# Shows old version or errors
```

**Cause**: Homebrew Rust installation conflicting with rustup

**Fix**:
```bash
# Remove Homebrew Rust
brew uninstall rust

# Ensure rustup Rust is first in PATH
cat >> ~/.zshenv << 'EOF'
# Rust from rustup (before Homebrew)
export PATH="$HOME/.cargo/bin:$PATH"
EOF

source ~/.zshenv

# Verify
which rustc
# Should show: /Users/<username>/.cargo/bin/rustc
```

---

### WSL2: Cannot Access Windows Files

**Symptoms**:
```bash
ls /mnt/c/Users/
# ls: cannot access '/mnt/c/Users/': No such file or directory
```

**Cause**: Windows drives not mounted

**Fix**:
```bash
# Restart WSL
# From PowerShell:
wsl --shutdown
wsl

# Check mounts
ls /mnt/
# Should show: c, d, etc.

# If still missing, check /etc/wsl.conf
sudo nano /etc/wsl.conf

# Should contain:
[automount]
enabled = true
root = /mnt/

# Save and restart WSL
```

---

### WSL2: systemd Not Available

**Symptoms**:
```bash
systemctl status
# System has not been booted with systemd
```

**Cause**: systemd not enabled in WSL2

**Fix** (Ubuntu 22.04+):
```bash
# Enable systemd
sudo nano /etc/wsl.conf

# Add:
[boot]
systemd=true

# Save and restart WSL
# From PowerShell:
wsl --shutdown
wsl

# Verify
systemctl status
```

**Note**: systemd not required for nabi, but useful for services

---

## Performance Issues

### Issue: Slow Startup Time

**Symptoms**:
```bash
time nabi --version
# real    0m2.000s (should be <0.1s)
```

**Cause**: Binary on slow filesystem (WSL2) or first run

**Fix (WSL2)**:
```bash
# Ensure binary on Linux filesystem
ls -la ~/.local/bin/nabi
# Should NOT show /mnt/c/...

# If on Windows filesystem, reinstall
make install
```

**Cause**: Antivirus scanning (Windows/macOS)

**Fix**:
```bash
# Add exclusion for ~/.local/bin/ in antivirus
# Or add exclusion for ~/.cache/nabi/ for build performance
```

---

### Issue: High Memory Usage During Build

**Symptoms**:
```bash
# During build, system becomes unresponsive
# WSL2: "out of memory" errors
```

**Fix (WSL2)**:
```bash
# Create or edit: C:\Users\<WindowsUsername>\.wslconfig
[wsl2]
memory=4GB
processors=2
swap=4GB

# Restart WSL
wsl --shutdown
wsl

# Retry build with limited parallelism
cd ~/nabia/core/nabi-cli
cargo build --release -j 1  # Single-threaded build
```

**Fix (M1 macOS)**:
```bash
# Close other applications
# Use Activity Monitor to check memory pressure
# If persistent, consider adding RAM or using cloud build
```

---

## Known Limitations

### 1. SurrealDB Migration Incomplete (61%)

**Impact**: Memory system falls back to file-based (memory.json)

**Symptoms**:
```bash
# Graph queries return partial results
# Federation events may not be tracked
```

**Status**: Known issue, migration in progress

**Workaround**: Use file-based memory (automatic fallback)

**When will this be fixed?**
- Completion of SurrealDB migration (future roadmap)
- See: `~/docs/architecture/memory-layer/THREE_TIER_MEMORY_MIGRATION.md`

---

### 2. Commander Plugins Not Implemented

**Impact**: `nabi claude`, `nabi data`, `nabi federation` show "not implemented"

**Status**: Foundation complete, commanders are future roadmap

**Workaround**: Use `nabi-python` shim or direct tools

**When will this be fixed?**
- Campaign Alpha (data-forge) - next priority
- Campaign Beta (claude-commander) - medium priority
- See: `CLAUDE.md` for roadmap

---

### 3. Federation Coordination Unverified on Fresh Installs

**Impact**: Agent coordination may need manual setup

**Status**: Needs testing on fresh installations

**Workaround**:
```bash
# Manually create federation directories
mkdir -p ~/.local/state/nabi/federation

# Verify federation registry
ls -la ~/.local/state/nabi/federation-registry.json
```

---

## Recovery Procedures

### Complete Uninstall and Fresh Start

**WARNING**: This deletes all nabi-related directories

```bash
# 1. Stop any running processes
pkill -f nabi

# 2. Remove XDG directories
rm -rf ~/.config/nabi
rm -rf ~/.cache/nabi
rm -rf ~/.local/share/nabi
rm -rf ~/.local/state/nabi

# 3. Remove binaries
rm -f ~/.local/bin/nabi
rm -f ~/.local/bin/nabi-python

# 4. Remove development directories (optional)
rm -rf ~/nabia/core/nabi-cli
rm -rf ~/nabia/tools/nabi-python

# 5. Remove unified access layer
rm -rf ~/.nabi

# 6. Remove environment variables
# M1 macOS: Edit ~/.zshenv and remove XDG exports
# WSL2 Ubuntu: Edit ~/.bashrc and remove XDG exports

# 7. Restart shell
# M1 macOS:
exec zsh

# WSL2 Ubuntu:
exec bash

# 8. Start fresh installation
# Follow deployment playbook from beginning
```

---

### Partial Reinstall (Keep Configuration)

```bash
# 1. Remove binary only
rm -f ~/.local/bin/nabi

# 2. Clean build artifacts
cd ~/nabia/core/nabi-cli
make clean

# 3. Rebuild and reinstall
make build
make install

# 4. Verify
nabi --version
nabi self doctor
```

---

### Rollback to Previous Build

```bash
# If you have a backup:
cp ~/.local/bin/nabi.backup ~/.local/bin/nabi

# Or rebuild from specific git commit:
cd ~/nabia/core/nabi-cli
git log  # Find working commit
git checkout <commit-hash>
make clean
make build
make install
```

---

## Getting Help

### Diagnostic Information to Collect

When reporting issues, include:

```bash
# 1. Platform information
uname -a
rustc --version --verbose
cargo --version
uv --version

# M1 macOS specific:
sw_vers
sysctl sysctl.proc_translated

# WSL2 Ubuntu specific:
lsb_release -a
# From PowerShell: wsl --list --verbose

# 2. nabi information
nabi --version
nabi self config
nabi self doctor

# 3. Environment
echo "XDG_CONFIG_HOME: $XDG_CONFIG_HOME"
echo "XDG_CACHE_HOME: $XDG_CACHE_HOME"
echo "XDG_DATA_HOME: $XDG_DATA_HOME"
echo "XDG_STATE_HOME: $XDG_STATE_HOME"
echo "PATH: $PATH"

# 4. Directory structure
ls -la ~/.config/nabi
ls -la ~/.cache/nabi
ls -la ~/.local/share/nabi
ls -la ~/.local/state/nabi
ls -la ~/.local/bin/nabi*

# 5. Build log (if build issue)
cd ~/nabia/core/nabi-cli
cargo build --release --verbose 2>&1 | tee build.log
# Attach build.log
```

---

### Resources

- Full M1 Playbook: `DEPLOYMENT_PLAYBOOK_M1_MACOS.md`
- Full WSL2 Playbook: `DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md`
- Quick Reference: `DEPLOYMENT_QUICK_REFERENCE.md`
- Architecture Docs: `~/docs/architecture/`

---

**Last Updated**: 2025-10-30
**Maintained By**: Nabia Federation
