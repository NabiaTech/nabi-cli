# NabiOS Deployment Documentation Index

**Purpose**: Master index for all deployment playbooks and guides
**Status**: Production Ready
**Last Updated**: 2025-10-30

---

## Quick Start

**Choose your platform**:
- [M1 MacBook Pro (macOS)](#m1-macos-deployment) - 45-60 minutes
- [WSL2 Ubuntu 22.04](#wsl2-ubuntu-deployment) - 40-50 minutes

**Already started? Having issues?**
- [Quick Reference](#quick-reference-guide) - Side-by-side platform comparison
- [Troubleshooting](#troubleshooting-guide) - Comprehensive issue resolution

---

## Document Overview

| Document | Purpose | Use When |
|----------|---------|----------|
| **[DEPLOYMENT_PLAYBOOK_M1_MACOS.md](DEPLOYMENT_PLAYBOOK_M1_MACOS.md)** | Full M1 macOS deployment guide | Installing on M1/M2/M3 MacBook |
| **[DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md](DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md)** | Full WSL2 Ubuntu deployment guide | Installing on Windows WSL2 |
| **[DEPLOYMENT_QUICK_REFERENCE.md](DEPLOYMENT_QUICK_REFERENCE.md)** | Side-by-side platform comparison | Quick command lookup |
| **[DEPLOYMENT_TROUBLESHOOTING.md](DEPLOYMENT_TROUBLESHOOTING.md)** | Comprehensive troubleshooting | Fixing installation issues |

---

## M1 macOS Deployment

**Document**: [DEPLOYMENT_PLAYBOOK_M1_MACOS.md](DEPLOYMENT_PLAYBOOK_M1_MACOS.md)

### At a Glance

- **Platform**: M1/M2/M3 MacBook Pro (ARM64)
- **OS Versions**: macOS Sonoma 14.x, Ventura 13.x
- **Installation Time**: 45-60 minutes
- **Difficulty**: Intermediate

### Prerequisites

- macOS 13.x (Ventura) or 14.x (Sonoma)
- 5GB+ available disk space
- Command line experience
- Internet connection

### What You'll Install

1. **Rust Toolchain** (ARM64 native)
   - rustc 1.88.0+
   - cargo package manager
   - ARM64 optimized compilation

2. **nabi-cli Binary** (Rust)
   - Layer 1 router (~2.0 MB)
   - XDG-compliant build system
   - System-wide installation

3. **Python Environment** (uv + Python 3.13)
   - Modern package management
   - Virtual environment support
   - Layer 3 tool ecosystem

4. **XDG Directory Structure**
   - ~/.config/nabi (configuration)
   - ~/.cache/nabi (build artifacts)
   - ~/.local/share/nabi (persistent data)
   - ~/.local/state/nabi (runtime state)

### Installation Phases

| Phase | Time | What Happens |
|-------|------|--------------|
| **1. System Dependencies** | 15 min | Rust, Python, XDG setup |
| **2. Build nabi Binary** | 20 min | Compile ARM64 binary |
| **3. Python CLI Layer** | 10 min | Install Python shim |
| **4. Verification** | 10 min | Run health checks |
| **5. Post-Install** | Optional | Shell completions, unified access |

### Success Criteria

After installation, these should work:
```bash
nabi --version          # Shows version 0.1.0
nabi self doctor        # Passes all checks
nabi self config        # Shows XDG paths
which nabi              # Shows ~/.local/bin/nabi
rustc --version         # Shows 1.88.0+
uv --version            # Shows 0.8.22+
```

### Known Issues (M1-Specific)

- **Rosetta 2 Emulation**: Ensure terminal runs native ARM64
- **Xcode Tools**: May need Command Line Tools for linker
- **LaunchAgents**: Use HOME expansion (`~/`) not XDG variables

---

## WSL2 Ubuntu Deployment

**Document**: [DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md](DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md)

### At a Glance

- **Platform**: WSL2 with Ubuntu 22.04 LTS (x86_64)
- **Windows Version**: Windows 10 (19041+) or Windows 11
- **Installation Time**: 40-50 minutes (+ WSL2 setup if needed)
- **Difficulty**: Intermediate

### Prerequisites

- Windows 10 (build 19041+) or Windows 11
- WSL2 enabled and installed
- Ubuntu 22.04 LTS distribution
- 5GB+ available disk space
- Command line experience

### What You'll Install

1. **Rust Toolchain** (x86_64 Linux)
   - rustc 1.88.0+
   - cargo package manager
   - Linux native compilation

2. **nabi-cli Binary** (Rust)
   - Layer 1 router (~2.0 MB)
   - XDG-compliant (native Linux)
   - Cross-platform compatible

3. **Python Environment** (uv + Python 3.13)
   - Modern package management
   - Virtual environment support
   - Layer 3 tool ecosystem

4. **XDG Directory Structure**
   - ~/.config/nabi (configuration)
   - ~/.cache/nabi (build artifacts)
   - ~/.local/share/nabi (persistent data)
   - ~/.local/state/nabi (runtime state)

### Installation Phases

| Phase | Time | What Happens |
|-------|------|--------------|
| **0. WSL2 Setup** | 10 min | Enable WSL2 (if needed) |
| **1. System Dependencies** | 10 min | Rust, Python, build tools |
| **2. Build nabi Binary** | 15 min | Compile x86_64 binary |
| **3. Python CLI Layer** | 8 min | Install Python shim |
| **4. Windows Interop** | 3 min | File sharing setup |
| **5. Verification** | 8 min | Run health checks |

### Success Criteria

After installation, these should work:
```bash
nabi --version          # Shows version 0.1.0
nabi self doctor        # Passes all checks
nabi self config        # Shows XDG paths
which nabi              # Shows ~/.local/bin/nabi
rustc --version         # Shows 1.88.0+
uv --version            # Shows 0.8.22+

# WSL-specific checks:
ls /mnt/c/Users/        # Access Windows files
pwd                     # Should be on Linux filesystem (~/...)
```

### Known Issues (WSL2-Specific)

- **Windows Filesystem**: Building on /mnt/c/ is 5-10x slower
- **WSL Version**: Must be WSL2, not WSL1
- **systemd**: Requires Ubuntu 22.04+ and manual enable
- **Resource Limits**: May need .wslconfig tuning

---

## Quick Reference Guide

**Document**: [DEPLOYMENT_QUICK_REFERENCE.md](DEPLOYMENT_QUICK_REFERENCE.md)

### Use This For

- Side-by-side platform command comparison
- Quick command lookup during installation
- Platform-specific considerations
- Common verification commands

### Key Sections

1. **Platform Detection** - Check your system
2. **Installation Time Comparison** - Plan your time
3. **Critical Differences** - Platform-specific setup
4. **Common Commands** - Command equivalents
5. **Verification** - Success criteria

### Quick Command Reference

| Task | M1 macOS | WSL2 Ubuntu |
|------|----------|-------------|
| **Check OS** | `sw_vers` | `lsb_release -a` |
| **Check Arch** | `uname -m` → `arm64` | `uname -m` → `x86_64` |
| **Shell Config** | `~/.zshenv` | `~/.bashrc` |
| **Install Rust** | Same: `curl ... \| sh` | Same: `curl ... \| sh` |
| **Build Tools** | `xcode-select --install` | `sudo apt install build-essential` |

---

## Troubleshooting Guide

**Document**: [DEPLOYMENT_TROUBLESHOOTING.md](DEPLOYMENT_TROUBLESHOOTING.md)

### Use This For

- Fixing installation failures
- Diagnosing build issues
- Resolving runtime errors
- Platform-specific problems
- Recovery procedures

### Key Sections

1. **Pre-Installation Issues** - Before you start
2. **Dependency Issues** - Rust, Python, tools
3. **Build Issues** - Compilation failures
4. **Installation Issues** - Binary installation
5. **Runtime Issues** - Post-installation problems
6. **Platform-Specific** - macOS/WSL2 unique issues
7. **Performance** - Slow builds, high memory
8. **Recovery** - Uninstall and fresh start

### Common Issues Quick Index

**Top 5 Issues**:
1. [`nabi: command not found`](DEPLOYMENT_TROUBLESHOOTING.md#issue-nabi-command-not-found) - PATH not set
2. [Build fails with "linker not found"](DEPLOYMENT_TROUBLESHOOTING.md#issue-build-fails-with-linker-cc-not-found) - C compiler missing
3. [`rustc: command not found`](DEPLOYMENT_TROUBLESHOOTING.md#issue-rustc-command-not-found-after-installation) - Rust env not sourced
4. [Build extremely slow](DEPLOYMENT_TROUBLESHOOTING.md#issue-build-extremely-slow-30-minutes) - Wrong filesystem or emulation
5. [Permission denied](DEPLOYMENT_TROUBLESHOOTING.md#issue-make-install-fails-with-permission-denied) - Directory permissions

**By Category**:
- **Build Failures**: See "Build Issues" section
- **Path Issues**: See "Installation Issues" section
- **Performance**: See "Performance Issues" section
- **Platform-Specific**: See "Platform-Specific Issues" section

---

## Installation Flow Chart

```
Start
  │
  ├─ Choose Platform
  │  ├─ M1 macOS → DEPLOYMENT_PLAYBOOK_M1_MACOS.md
  │  └─ WSL2 Ubuntu → DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md
  │
  ├─ Follow Playbook Phases
  │  ├─ Prerequisites Check
  │  ├─ Install Dependencies
  │  ├─ Build nabi Binary
  │  ├─ Install Binary
  │  └─ Run Verification
  │
  ├─ Success? ──Yes→ Done ✅
  │         │
  │        No
  │         ↓
  │    DEPLOYMENT_TROUBLESHOOTING.md
  │         │
  │    Fix Issues
  │         │
  │    Retry or Recovery
  │         │
  └─────────┘
```

---

## What Works After Installation

### Core Functionality ✅

```bash
# Self-management commands (Layer 1 Rust)
nabi --version              # Version info
nabi --help                 # Help text
nabi self doctor            # Health checks
nabi self config            # Show configuration

# Repository validation (Layer 1 Rust)
nabi repo check             # Compliance checks
nabi repo analyze           # Repository analysis

# Tool management (Layer 1 Rust)
nabi tool list              # List registered tools
nabi tool resolve <name>    # Resolve tool paths
```

### Python Layer ✅ (if installed)

```bash
# Python CLI shim (Layer 2 Bash)
nabi-python --help          # Show Python commands
nabi-python <command>       # Route to Python tools

# Individual tools (Layer 3 Python)
# (if installed separately)
```

### Commander Plugins ⏳ (Future)

```bash
# These show "not implemented" (expected):
nabi claude session list    # ⏳ Commander pending
nabi data jsonl validate    # ⏳ Commander pending
nabi federation agent list  # ⏳ Commander pending
```

**Status**: Foundation complete, commanders are future roadmap

---

## Known Limitations

### 1. SurrealDB Migration Incomplete

- **Status**: 61% complete (496/804 entities)
- **Impact**: Memory system falls back to file-based (memory.json)
- **Workaround**: Automatic fallback, no action required

### 2. Commander Plugins Not Implemented

- **Status**: Foundation complete, commanders pending
- **Impact**: `nabi claude/data/federation` commands not yet functional
- **Workaround**: Use `nabi-python` shim or direct tools

### 3. Federation Coordination Unverified

- **Status**: Architecture in place, needs testing
- **Impact**: Fresh install federation may need manual setup
- **Workaround**: Create federation directories manually

### 4. Platform-Specific

**M1 macOS**:
- LaunchAgents require HOME expansion, not XDG variables
- Rosetta 2 can cause slow builds if terminal is emulated

**WSL2 Ubuntu**:
- Windows filesystem (/mnt/c/) is 5-10x slower for builds
- Must be WSL2, not WSL1
- systemd requires manual enablement

---

## Post-Installation Next Steps

### 1. Install Additional Tools (Optional)

```bash
# Example: riff-cli (search and archive)
cd ~/nabia/tools
git clone <riff-cli-repo>
cd riff-cli
uv venv ~/.local/share/nabi/venvs/riff-cli
uv pip install --venv ~/.local/share/nabi/venvs/riff-cli -e .
```

### 2. Configure Federation (When Ready)

```bash
# Create federation directories
mkdir -p ~/.local/state/nabi/federation

# Check federation registry
ls -la ~/.local/state/nabi/federation-registry.json
```

### 3. Create Unified Access Layer

```bash
# Optional: convenience symlinks
mkdir -p ~/.nabi
ln -sf ~/.local/state/nabi ~/.nabi/state
ln -sf ~/.local/share/nabi ~/.nabi/share
ln -sf ~/.cache/nabi ~/.nabi/cache
ln -sf ~/.config/nabi ~/.nabi/config
```

### 4. Set Up Shell Enhancements

```bash
# Shell completions (optional)
# M1 macOS (zsh):
nabi completions zsh > ~/.local/share/zsh/site-functions/_nabi

# WSL2 Ubuntu (bash):
nabi completions bash > ~/.local/share/bash-completion/completions/nabi
```

---

## Verification Checklist

After installation, verify these items:

### Basic Installation ✅

- [ ] `nabi --version` shows version 0.1.0
- [ ] `nabi self doctor` passes all checks
- [ ] `nabi self config` shows correct XDG paths
- [ ] `which nabi` shows ~/.local/bin/nabi
- [ ] Build artifacts in ~/.cache/nabi/ (not in source)

### Dependencies ✅

- [ ] `rustc --version` shows 1.70+
- [ ] `cargo --version` shows 1.70+
- [ ] `uv --version` shows 0.8.22+
- [ ] Rust environment sourced (in shell config)

### XDG Compliance ✅

- [ ] ~/.config/nabi exists
- [ ] ~/.cache/nabi exists
- [ ] ~/.local/share/nabi exists
- [ ] ~/.local/state/nabi exists
- [ ] XDG_* environment variables set

### Platform-Specific ✅

**M1 macOS**:
- [ ] `uname -m` shows `arm64`
- [ ] `rustc --version --verbose | grep host` shows `aarch64-apple-darwin`
- [ ] Not running under Rosetta 2 emulation

**WSL2 Ubuntu**:
- [ ] `uname -m` shows `x86_64`
- [ ] `rustc --version --verbose | grep host` shows `x86_64-unknown-linux-gnu`
- [ ] WSL version is 2 (not 1)
- [ ] Installation on Linux filesystem (not /mnt/c/)

---

## Getting Help

### Documentation

- **Architecture**: `~/docs/architecture/UNIFIED_CONVERGENCE_PLAN.md`
- **CLI Reference**: `~/docs/tools/nabi-cli.md`
- **XDG Topology**: `~/docs/architecture/XDG_TOPOLOGY_SINGLE_SPAN.md`

### Commands

```bash
nabi --help                 # General help
nabi self doctor --help     # Doctor command help
nabi repo check --help      # Repo check help
```

### Diagnostic Information

When reporting issues, collect:

```bash
# Platform info
uname -a
rustc --version --verbose
cargo --version
uv --version

# M1 macOS:
sw_vers

# WSL2 Ubuntu:
lsb_release -a

# nabi info
nabi --version
nabi self config
nabi self doctor

# Environment
echo "XDG_CONFIG_HOME: $XDG_CONFIG_HOME"
echo "PATH: $PATH"

# Directory structure
ls -la ~/.config/nabi
ls -la ~/.cache/nabi
ls -la ~/.local/bin/nabi*
```

---

## Document Maintenance

### Version History

| Date | Version | Changes |
|------|---------|---------|
| 2025-10-30 | 1.0.0 | Initial release |

### Contributing

To update these playbooks:

1. Test changes on both platforms (M1 macOS and WSL2 Ubuntu)
2. Update relevant playbook(s)
3. Update this index if structure changes
4. Update version history
5. Test installation from scratch
6. Submit for review

### Feedback

If you find issues or have improvements:
- Document the issue with platform details
- Include reproduction steps
- Suggest fixes or improvements
- Test proposed changes

---

## Summary

This deployment documentation provides:

- ✅ **Complete installation guides** for M1 macOS and WSL2 Ubuntu
- ✅ **Quick reference** for platform-specific commands
- ✅ **Comprehensive troubleshooting** for common issues
- ✅ **Clear success criteria** for verification
- ✅ **Known limitations** and workarounds
- ✅ **Post-installation guidance** for next steps

**Total Time Investment**:
- M1 macOS: 45-60 minutes
- WSL2 Ubuntu: 40-50 minutes (+ WSL2 setup if needed)

**Difficulty Level**: Intermediate (requires command line experience)

**Outcome**: Fully functional nabi-cli federation gateway with:
- Rust-based CLI router (Layer 1)
- Bash routing layer (Layer 2)
- Python tool ecosystem ready (Layer 3)
- XDG-compliant directory structure
- Cross-platform compatibility

---

**Start Here**: Choose your platform and follow the corresponding playbook
- [M1 MacBook Pro](DEPLOYMENT_PLAYBOOK_M1_MACOS.md)
- [WSL2 Ubuntu 22.04](DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md)

**Need Help?** Check the [Troubleshooting Guide](DEPLOYMENT_TROUBLESHOOTING.md)

**Last Updated**: 2025-10-30
**Maintained By**: Nabia Federation
