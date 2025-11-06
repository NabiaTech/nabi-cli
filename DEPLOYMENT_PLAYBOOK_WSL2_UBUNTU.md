# NabiOS Deployment Playbook: WSL2 Ubuntu 22.04

**Target Platform**: WSL2 with Ubuntu 22.04 LTS (x86_64 architecture)
**Windows Version**: Windows 10 (build 19041+) or Windows 11
**Total Time**: 40-50 minutes (clean installation)
**Status**: Production Ready
**Last Updated**: 2025-10-30

---

## Executive Summary

This playbook deploys the **nabi-cli federation gateway** on WSL2 Ubuntu 22.04, establishing:
- ✅ Rust-based CLI router (Layer 1)
- ✅ Bash routing layer (Layer 2)
- ✅ Python tool ecosystem (Layer 3)
- ✅ XDG-compliant directory structure (native Linux)
- ⏳ Basic federation capabilities (coordination, memory)

**What Works After Installation**:
- `nabi self doctor` - System health checks
- `nabi self config` - Configuration display
- `nabi repo check` - Repository validation
- `nabi --version` - Version information
- Python CLI tools via `nabi-python` shim
- Windows/Linux interop (accessing Windows files)

**Known Limitations**:
- SurrealDB migration incomplete (61% - 496/804 entities)
- Federation coordination unverified on fresh installs
- Memory system falls back to file-based (memory.json)
- Commander plugins not yet implemented (future roadmap)
- Windows filesystem performance (use Linux filesystem for best performance)

---

## Prerequisites Check

**Run these commands in PowerShell (as Administrator) BEFORE starting**:

### Windows Side: Enable WSL2

```powershell
# 1. Check Windows version (must be build 19041+)
winver
# Or:
systeminfo | findstr /B /C:"OS Version"

# Expected: Windows 10 (19041+) or Windows 11

# 2. Enable WSL2 (if not already enabled)
wsl --install

# This installs:
# - WSL2 kernel
# - Ubuntu 22.04 (default distribution)
# - Virtual Machine Platform

# 3. Verify WSL2 is installed
wsl --list --verbose

# Expected output:
#   NAME            STATE           VERSION
# * Ubuntu-22.04    Stopped         2

# If VERSION shows 1, upgrade to WSL2:
wsl --set-version Ubuntu-22.04 2
wsl --set-default-version 2

# 4. Launch Ubuntu
wsl
```

**Expected First Launch**:
- Create Unix username and password
- This becomes your WSL user account

**Estimated Time**: 10 minutes (if WSL2 not installed)

### Linux Side: Verify Ubuntu

```bash
# Now you're in Ubuntu bash shell

# 1. Check Ubuntu version (must be 22.04 LTS)
lsb_release -a

# Expected output:
# Description:    Ubuntu 22.04.x LTS
# Release:        22.04
# Codename:       jammy

# 2. Check CPU architecture (must be x86_64)
uname -m

# Expected output: x86_64

# 3. Check available disk space (need at least 5GB)
df -h ~

# Expected output: At least 5GB available

# 4. Update package lists
sudo apt update

# 5. Install basic dependencies
sudo apt install -y build-essential curl git
```

**Estimated Time**: 5 minutes

---

## Phase 1: System Dependencies (10 minutes)

### Step 1.1: Install Rust Toolchain for x86_64

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

To configure your current shell, source the corresponding env file:
  source "$HOME/.cargo/env"
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

# Verify x86_64 target
rustc --version --verbose | grep host

# Expected: host: x86_64-unknown-linux-gnu
```

**Estimated Time**: 3 minutes

### Step 1.2: Install Python with uv

```bash
# Install uv (modern Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Source the environment
source "$HOME/.cargo/env"

# Verify installation
uv --version

# Expected: uv 0.8.22 (or newer)

# Install Python 3.13 (recommended)
uv python install 3.13

# Verify Python installation
uv python list

# Expected output should include: python3.13
```

**Estimated Time**: 3 minutes

### Step 1.3: Set Up XDG Environment Variables

**Good News**: Ubuntu 22.04 follows XDG Base Directory Specification natively!

```bash
# Verify XDG variables are already set
echo "XDG_CONFIG_HOME: ${XDG_CONFIG_HOME:-$HOME/.config}"
echo "XDG_CACHE_HOME: ${XDG_CACHE_HOME:-$HOME/.cache}"
echo "XDG_DATA_HOME: ${XDG_DATA_HOME:-$HOME/.local/share}"
echo "XDG_STATE_HOME: ${XDG_STATE_HOME:-$HOME/.local/state}"

# Expected output:
# XDG_CONFIG_HOME: /home/<username>/.config
# XDG_CACHE_HOME: /home/<username>/.cache
# XDG_DATA_HOME: /home/<username>/.local/share
# XDG_STATE_HOME: /home/<username>/.local/state

# Add to ~/.bashrc for persistence
cat >> ~/.bashrc << 'EOF'

# XDG Base Directory Specification
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"

# Add ~/.local/bin to PATH for nabi binary
export PATH="$HOME/.local/bin:$PATH"

# Rust environment
source "$HOME/.cargo/env"

EOF

# Apply changes
source ~/.bashrc

# Verify
echo $PATH | grep ".local/bin"
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

## Phase 2: Install nabi-cli Binary (15 minutes)

### Step 2.1: Clone nabi-cli Source

```bash
# Clone to development directory
cd ~/nabia/core
git clone https://github.com/nabia-federation/nabi-cli.git

# If repository is private or doesn't exist yet, you may need to:
# 1. Request access
# 2. Copy from existing installation (see Windows interop below)
# 3. Use local tarball

# For now, assuming source is at: ~/nabia/core/nabi-cli/
cd ~/nabia/core/nabi-cli
```

**Windows/Linux Interop (if copying from Windows)**:
```bash
# Windows files are accessible at /mnt/c/, /mnt/d/, etc.
# Example: Copy from Windows Downloads folder
cp -r /mnt/c/Users/<WindowsUsername>/Downloads/nabi-cli ~/nabia/core/nabi-cli
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
# target-dir = "/home/<username>/.cache/nabi/nabi-cli/target"
```

**Troubleshooting**:
If `make config` fails:
```bash
# Install make if not present
sudo apt install -y make

# Or manually create the config
mkdir -p .cargo
sed "s|\$XDG_CACHE_HOME|$HOME/.cache|g" .cargo/config.toml.template > .cargo/config.toml
```

**Estimated Time**: 1 minute

### Step 2.3: Build nabi Binary (x86_64)

```bash
# Build release binary (optimized for x86_64)
cd ~/nabia/core/nabi-cli
make build

# This will:
# 1. Download and compile dependencies (first time: 8-12 minutes)
# 2. Compile nabi source code
# 3. Output to: ~/.cache/nabi/nabi-cli/target/release/nabi

# You'll see output like:
#    Compiling nabi v0.1.0 (/home/<username>/nabia/core/nabi-cli)
#    Finished release [optimized] target(s) in 6m 24s
```

**Expected Build Time**:
- First build: 8-12 minutes (downloads dependencies)
- Subsequent builds: 30-45 seconds (incremental)

**Troubleshooting**:
If build fails with "could not compile":
```bash
# Check Rust version (must be 1.70+)
rustc --version

# Check cargo is working
cargo --version

# Install additional dependencies if needed
sudo apt install -y pkg-config libssl-dev

# Try cleaning and rebuilding
make clean
make build
```

**Estimated Time**: 12 minutes (first build)

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
# -rwxr-xr-x 1 <user> <user> 2.0M Oct 30 22:47 /home/<username>/.local/bin/nabi

# Test binary
which nabi

# Expected: /home/<username>/.local/bin/nabi

# Test version
nabi --version

# Expected: nabi 0.1.0
```

**Troubleshooting**:
If `which nabi` doesn't find it:
```bash
# Check PATH includes ~/.local/bin
echo $PATH | grep ".local/bin"

# If not found, add to ~/.bashrc:
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

**Estimated Time**: 2 minutes

---

## Phase 3: Python CLI Layer (8 minutes)

### Step 3.1: Install nabi-python Shim

```bash
# Create tools directory if it doesn't exist
mkdir -p ~/nabia/tools

# Copy or symlink nabi-python shim
# Option A: If you have existing nabi-python (from Windows):
cp /mnt/c/Users/<WindowsUsername>/path/to/nabi-python ~/nabia/tools/nabi-python
chmod +x ~/nabia/tools/nabi-python

# Option B: Create symlink in ~/.local/bin
ln -sf ~/nabia/tools/nabi-python ~/.local/bin/nabi-python

# Verify installation
ls -la ~/.local/bin/nabi-python

# Expected: symlink to ~/nabia/tools/nabi-python or standalone file

# Test shim
nabi-python --help

# Expected: Usage information or command list
```

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

## Phase 4: Windows/Linux Interop Setup (Optional)

### Step 4.1: Access Windows Files from WSL

```bash
# Windows drives are mounted at /mnt/
ls /mnt/c/Users/

# Example: Access Windows Documents
cd /mnt/c/Users/<WindowsUsername>/Documents

# Create symbolic link to Windows workspace (if desired)
ln -s /mnt/c/Users/<WindowsUsername>/workspace ~/windows-workspace

# NOTE: Keep nabi installation on Linux filesystem for best performance
# Only use Windows filesystem for data exchange
```

### Step 4.2: Access WSL Files from Windows

**From Windows Explorer**:
1. Type `\\wsl$\Ubuntu-22.04\home\<username>` in address bar
2. You can access all WSL files from Windows

**From Windows Terminal**:
```powershell
# Access WSL home directory
cd \\wsl$\Ubuntu-22.04\home\<username>
```

**Best Practice**:
- Keep nabi installation in WSL filesystem (`~/nabia/`)
- Use Windows filesystem for sharing files between Windows and WSL
- Avoid building Rust projects on Windows filesystem (slow)

**Estimated Time**: 3 minutes

---

## Phase 5: Verification & Testing (8 minutes)

### Step 5.1: Run Health Check

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

### Step 5.2: Test Core Commands

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

**Estimated Time**: 2 minutes

### Step 5.3: Verify XDG Compliance

```bash
# Check all XDG paths are being used
nabi self config | grep -E "(CONFIG|CACHE|DATA|STATE)_HOME"

# Expected output should show:
# XDG_CONFIG_HOME: /home/<username>/.config
# XDG_CACHE_HOME: /home/<username>/.cache
# XDG_DATA_HOME: /home/<username>/.local/share
# XDG_STATE_HOME: /home/<username>/.local/state

# Verify build artifacts are in cache
ls -la ~/.cache/nabi/nabi-cli/target/

# Expected: Cargo build artifacts (target/ directory)

# Verify no artifacts in source directory
ls -la ~/nabia/core/nabi-cli/ | grep target

# Expected: Only .cargo/config.toml (no target/ directory)
```

**Estimated Time**: 2 minutes

### Step 5.4: Test Python Layer (if installed)

```bash
# Test nabi-python shim
nabi-python --help

# If riff-cli or other tools are installed:
# nabi-python riff --version

# Test Layer 2 routing
# (This depends on what commanders are installed)
```

**Estimated Time**: 2 minutes

---

## Phase 6: Post-Installation Setup (Optional)

### Step 6.1: Create Unified Access Layer

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

### Step 6.2: Enable Shell Completions (Optional)

```bash
# Generate shell completions for bash
nabi completions bash > ~/.local/share/bash-completion/completions/nabi

# Or add to .bashrc:
echo 'eval "$(nabi completions bash)"' >> ~/.bashrc
source ~/.bashrc

# Test tab completion
nabi self <TAB>
# Expected: Shows available subcommands (doctor, config, etc.)
```

**Estimated Time**: 2 minutes

---

## WSL-Specific Considerations

### Performance Best Practices

1. **Use Linux Filesystem for Development**
   ```bash
   # ✅ FAST - Linux filesystem
   ~/nabia/core/nabi-cli/

   # ❌ SLOW - Windows filesystem (via /mnt/c/)
   /mnt/c/Users/<user>/projects/nabi-cli/
   ```

2. **Build on Linux Filesystem**
   ```bash
   # Building on Windows filesystem is 5-10x slower
   # Always build in ~/nabia/ or similar
   ```

3. **Use WSL 2, Not WSL 1**
   ```bash
   # Check WSL version
   wsl --list --verbose

   # If VERSION is 1, upgrade:
   wsl --set-version Ubuntu-22.04 2
   ```

### Memory and Resource Limits

```bash
# Create .wslconfig in Windows user directory
# From PowerShell:
notepad C:\Users\<WindowsUsername>\.wslconfig

# Add these settings:
[wsl2]
memory=4GB
processors=2
swap=2GB

# Restart WSL for changes to take effect
wsl --shutdown
wsl
```

### Networking

```bash
# WSL2 uses NAT networking
# Access Windows localhost from WSL:
curl http://localhost:8080  # Works

# Access WSL localhost from Windows:
# Use WSL IP address or localhost (Windows 11+)
ip addr show eth0 | grep inet

# Port forwarding is automatic in WSL2
```

---

## Success Criteria

### ✅ Minimum Viable Installation

After completing Phases 1-5, you should have:

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

### 4. Windows Filesystem Performance

**Status**: WSL2 limitation

**Impact**:
- Building on /mnt/c/ is 5-10x slower
- File watching may be unreliable on Windows filesystem

**Workaround**:
```bash
# Always build on Linux filesystem
cd ~/nabia/core/nabi-cli  # ✅ Good
cd /mnt/c/projects/nabi-cli  # ❌ Slow
```

---

## Troubleshooting Guide

### Issue: `nabi: command not found`

**Cause**: PATH doesn't include ~/.local/bin

**Fix**:
```bash
# Add to ~/.bashrc
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify
which nabi
```

### Issue: Build fails with "linker not found"

**Cause**: Build tools not installed

**Fix**:
```bash
# Install build essentials
sudo apt update
sudo apt install -y build-essential

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
echo 'source "$HOME/.cargo/env"' >> ~/.bashrc
```

### Issue: Permission denied on ~/.local/bin/nabi

**Cause**: Binary not executable

**Fix**:
```bash
chmod +x ~/.local/bin/nabi
```

### Issue: WSL is very slow

**Cause**: Using Windows filesystem or WSL1

**Fix**:
```bash
# 1. Check WSL version
wsl --list --verbose

# If VERSION is 1, upgrade to WSL2:
# From PowerShell (as Administrator):
wsl --set-version Ubuntu-22.04 2

# 2. Move project to Linux filesystem
cd ~
mv /mnt/c/projects/nabi-cli ~/nabia/core/nabi-cli

# 3. Configure WSL2 memory (see WSL-Specific Considerations)
```

### Issue: Cannot access Windows files from WSL

**Cause**: Drive not mounted

**Fix**:
```bash
# Check mounted drives
ls /mnt/

# Should show: c, d, etc.

# If missing, restart WSL:
# From PowerShell:
wsl --shutdown
wsl
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

# Remove environment variables from ~/.bashrc
# (manually edit and remove XDG_* exports)

# Restart shell
exec bash
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

5. **Windows Integration** (Optional)
   - Install Windows Terminal for better experience
   - Configure VS Code Remote-WSL
   - Set up file sharing between Windows and WSL

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

**WSL-Specific Resources**:
- Microsoft WSL Documentation: https://docs.microsoft.com/en-us/windows/wsl/
- WSL GitHub Issues: https://github.com/microsoft/WSL/issues

**Common Issues**:
- Check GitHub issues: (if repository is public)
- Check federation docs: `~/docs/federation/`

---

**Deployment Status**: ✅ Ready for Production
**Platform**: WSL2 Ubuntu 22.04 (x86_64)
**Installation Time**: 40-50 minutes
**Difficulty**: Intermediate (requires command line experience)
