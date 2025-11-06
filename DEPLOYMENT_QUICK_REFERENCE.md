# NabiOS Deployment Quick Reference

**Purpose**: Side-by-side comparison for M1 macOS and WSL2 Ubuntu deployments
**Use Case**: Quick lookup for platform-specific commands
**Last Updated**: 2025-10-30

---

## Platform Detection

| Check | M1 macOS | WSL2 Ubuntu |
|-------|----------|-------------|
| **OS Version** | `sw_vers` | `lsb_release -a` |
| **Architecture** | `uname -m` → `arm64` | `uname -m` → `x86_64` |
| **Disk Space** | `df -h ~` | `df -h ~` |
| **Shell** | `zsh` (default) | `bash` (default) |

---

## Installation Time Comparison

| Phase | M1 macOS | WSL2 Ubuntu |
|-------|----------|-------------|
| **Prerequisites** | 5 min | 15 min (if WSL2 not installed) |
| **Dependencies** | 15 min | 10 min |
| **Build nabi** | 20 min | 15 min |
| **Python Layer** | 10 min | 8 min |
| **Verification** | 10 min | 8 min |
| **Total** | **45-60 min** | **40-50 min** (56-66 min if WSL2 install needed) |

---

## Critical Differences

### 1. XDG Variables

| Aspect | M1 macOS | WSL2 Ubuntu |
|--------|----------|-------------|
| **Native Support** | ❌ No (forced via shell) | ✅ Yes (native Linux) |
| **Config File** | `~/.zshenv` | `~/.bashrc` |
| **PATH Setup** | Manual (add to `~/.zshenv`) | Often auto-configured |

**M1 macOS Setup**:
```bash
# Add to ~/.zshenv
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export PATH="$HOME/.local/bin:$PATH"
source "$HOME/.cargo/env"
```

**WSL2 Ubuntu Setup**:
```bash
# Add to ~/.bashrc
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export PATH="$HOME/.local/bin:$PATH"
source "$HOME/.cargo/env"
```

### 2. Build Dependencies

| Tool | M1 macOS | WSL2 Ubuntu |
|------|----------|-------------|
| **C Compiler** | Xcode Command Line Tools | `build-essential` |
| **Install Command** | `xcode-select --install` | `sudo apt install -y build-essential` |
| **Package Manager** | Homebrew (optional) | apt |
| **SSL Libraries** | Usually pre-installed | `libssl-dev` (may be needed) |

### 3. Rust Target Architecture

| Platform | Target Triple | Binary Size |
|----------|---------------|-------------|
| **M1 macOS** | `aarch64-apple-darwin` | ~2.0 MB |
| **WSL2 Ubuntu** | `x86_64-unknown-linux-gnu` | ~2.0 MB |

**Verify Target**:
```bash
# Both platforms
rustc --version --verbose | grep host

# M1 macOS output:
# host: aarch64-apple-darwin

# WSL2 Ubuntu output:
# host: x86_64-unknown-linux-gnu
```

---

## Common Commands by Platform

### Rust Installation

| Step | M1 macOS | WSL2 Ubuntu |
|------|----------|-------------|
| **Install** | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Source** | `source "$HOME/.cargo/env"` | `source "$HOME/.cargo/env"` |
| **Persist** | Add to `~/.zshenv` | Add to `~/.bashrc` |

### Python/uv Installation

| Step | M1 macOS | WSL2 Ubuntu |
|------|----------|-------------|
| **Install uv** | `curl -LsSf https://astral.sh/uv/install.sh \| sh` | `curl -LsSf https://astral.sh/uv/install.sh \| sh` |
| **Install Python** | `uv python install 3.13` | `uv python install 3.13` |
| **Verify** | `uv python list` | `uv python list` |

### Build nabi-cli

| Step | M1 macOS | WSL2 Ubuntu |
|------|----------|-------------|
| **Clone** | `git clone <repo> ~/nabia/core/nabi-cli` | `git clone <repo> ~/nabia/core/nabi-cli` |
| **Generate Config** | `make config` | `make config` |
| **Build** | `make build` | `make build` |
| **Install** | `make install` | `make install` |
| **Build Time** | 10-15 min (first) | 8-12 min (first) |

### Shell Reload

| Action | M1 macOS | WSL2 Ubuntu |
|--------|----------|-------------|
| **Reload Shell** | `source ~/.zshenv` or `exec zsh` | `source ~/.bashrc` or `exec bash` |
| **Check PATH** | `echo $PATH \| grep .local/bin` | `echo $PATH \| grep .local/bin` |

---

## Verification Commands (Identical)

Both platforms use the same verification commands:

```bash
# Check nabi installation
which nabi
nabi --version
nabi self doctor
nabi self config

# Check XDG directories
ls -la ~/.config/nabi
ls -la ~/.cache/nabi
ls -la ~/.local/share/nabi
ls -la ~/.local/state/nabi

# Check Rust
rustc --version
cargo --version

# Check Python
uv --version
uv python list

# Check build artifacts location
ls -la ~/.cache/nabi/nabi-cli/target/

# Check no artifacts in source
ls -la ~/nabia/core/nabi-cli/ | grep target
# Should only show: .cargo/config.toml
```

---

## Platform-Specific Considerations

### M1 macOS Only

**Rosetta 2** (not needed for nabi, but FYI):
- nabi builds native ARM64 binaries
- No emulation required
- If you see `x86_64` target, you're running under Rosetta

**Xcode Command Line Tools**:
```bash
# If build fails with "linker not found"
xcode-select --install

# Verify
xcode-select -p
# Expected: /Library/Developer/CommandLineTools
```

**macOS-specific paths**:
- LaunchAgents: `~/Library/LaunchAgents/` (not `~/.config/systemd/`)
- Application Support: `~/Library/Application Support/` (native macOS)
- Logs: `~/Library/Logs/` (native macOS)

**Note**: nabi uses XDG paths for cross-platform compatibility, not macOS native paths.

### WSL2 Ubuntu Only

**Windows Interop**:
```bash
# Access Windows drives
ls /mnt/c/Users/<WindowsUsername>/

# Access WSL from Windows
# In Windows Explorer: \\wsl$\Ubuntu-22.04\home\<username>

# Copy from Windows to WSL
cp /mnt/c/Users/<user>/Downloads/file.txt ~/

# Copy from WSL to Windows
cp ~/file.txt /mnt/c/Users/<user>/Downloads/
```

**Performance**:
```bash
# ✅ FAST - Use Linux filesystem
~/nabia/core/nabi-cli/

# ❌ SLOW - Avoid Windows filesystem for builds
/mnt/c/projects/nabi-cli/  # 5-10x slower!
```

**WSL Version Check**:
```powershell
# From PowerShell
wsl --list --verbose

# Should show VERSION: 2
# If VERSION: 1, upgrade:
wsl --set-version Ubuntu-22.04 2
```

**Resource Limits**:
```ini
# Create C:\Users\<WindowsUsername>\.wslconfig
[wsl2]
memory=4GB
processors=2
swap=2GB

# Restart WSL
wsl --shutdown
```

**systemd Support** (Ubuntu 22.04):
```bash
# Enable systemd in WSL2 (optional)
sudo nano /etc/wsl.conf

# Add:
[boot]
systemd=true

# Restart WSL from PowerShell
wsl --shutdown
wsl
```

---

## Common Issues and Platform-Specific Fixes

### Issue: `nabi: command not found`

| Platform | Fix |
|----------|-----|
| **M1 macOS** | `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshenv && source ~/.zshenv` |
| **WSL2 Ubuntu** | `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc` |

### Issue: Build fails with "linker not found"

| Platform | Fix |
|----------|-----|
| **M1 macOS** | `xcode-select --install` |
| **WSL2 Ubuntu** | `sudo apt install -y build-essential` |

### Issue: `rustc: command not found`

| Platform | Fix |
|----------|-----|
| **M1 macOS** | `source "$HOME/.cargo/env"` then add to `~/.zshenv` |
| **WSL2 Ubuntu** | `source "$HOME/.cargo/env"` then add to `~/.bashrc` |

### Issue: Permission denied

| Platform | Fix |
|----------|-----|
| **Both** | `chmod +x ~/.local/bin/nabi` |

### Issue: Build very slow

| Platform | Cause | Fix |
|----------|-------|-----|
| **M1 macOS** | Running under Rosetta | Verify `uname -m` shows `arm64` |
| **WSL2 Ubuntu** | Building on Windows filesystem | Move to `~/nabia/` (Linux filesystem) |

---

## Post-Installation Checklist

**Both Platforms**:
- [ ] `nabi --version` works
- [ ] `nabi self doctor` passes
- [ ] `nabi self config` shows correct XDG paths
- [ ] Build artifacts in `~/.cache/nabi/` (not in source)
- [ ] `rustc --version` shows 1.70+
- [ ] `uv --version` works
- [ ] `which nabi` shows `~/.local/bin/nabi`

**M1 macOS Specific**:
- [ ] `uname -m` shows `arm64`
- [ ] `rustc --version --verbose | grep host` shows `aarch64-apple-darwin`
- [ ] Xcode Command Line Tools installed (if needed)

**WSL2 Ubuntu Specific**:
- [ ] `uname -m` shows `x86_64`
- [ ] `rustc --version --verbose | grep host` shows `x86_64-unknown-linux-gnu`
- [ ] WSL version is 2 (not 1)
- [ ] Installation on Linux filesystem (not `/mnt/c/`)

---

## Next Steps (Same for Both Platforms)

1. **Install Python Tools**
   ```bash
   # Example: riff-cli
   cd ~/nabia/tools
   git clone <riff-cli-repo>
   cd riff-cli
   uv venv ~/.local/share/nabi/venvs/riff-cli
   uv pip install --venv ~/.local/share/nabi/venvs/riff-cli -e .
   ```

2. **Configure Federation** (when ready)
   ```bash
   mkdir -p ~/.local/state/nabi/federation
   # Set up federation registry
   # Connect to coordination server
   ```

3. **Create Unified Access Layer**
   ```bash
   mkdir -p ~/.nabi
   ln -sf ~/.local/state/nabi ~/.nabi/state
   ln -sf ~/.local/share/nabi ~/.nabi/share
   ln -sf ~/.cache/nabi ~/.nabi/cache
   ln -sf ~/.config/nabi ~/.nabi/config
   ```

---

## Success Metrics

| Metric | M1 macOS | WSL2 Ubuntu |
|--------|----------|-------------|
| **Binary Size** | ~2.0 MB | ~2.0 MB |
| **Build Time (first)** | 10-15 min | 8-12 min |
| **Build Time (incremental)** | 30-60 sec | 30-45 sec |
| **Installation Time** | 45-60 min | 40-50 min |

---

## Getting Help

**Commands**:
```bash
# Both platforms
nabi --help
nabi self doctor --help
nabi repo check --help
```

**Documentation**:
- Full M1 playbook: `DEPLOYMENT_PLAYBOOK_M1_MACOS.md`
- Full WSL2 playbook: `DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md`
- Architecture: `~/docs/architecture/UNIFIED_CONVERGENCE_PLAN.md`
- CLI Reference: `~/docs/tools/nabi-cli.md`

---

**Last Updated**: 2025-10-30
**Maintained By**: Nabia Federation
