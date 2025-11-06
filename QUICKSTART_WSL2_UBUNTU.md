# Quick Start: WSL2 Ubuntu 22.04 (30-Second Overview)

**Time**: 40-50 minutes | **Platform**: WSL2 Ubuntu 22.04 (x86_64) | **Difficulty**: Intermediate

---

## Prerequisites (5 minutes)

**From PowerShell (Administrator)**:
```powershell
wsl --install                # If WSL2 not installed
wsl --list --verbose         # Verify VERSION: 2
wsl                          # Launch Ubuntu
```

**From Ubuntu bash**:
```bash
# Check you have:
lsb_release -a               # Ubuntu 22.04 LTS
uname -m                     # x86_64
df -h ~                      # 5GB+ free

# Install basics
sudo apt update
sudo apt install -y build-essential curl git
```

---

## Installation (5 Commands)

```bash
# 1. Install Rust (3 min)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 2. Install Python/uv (3 min)
curl -LsSf https://astral.sh/uv/install.sh | sh
uv python install 3.13

# 3. Set up XDG (1 min)
cat >> ~/.bashrc << 'EOF'
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export PATH="$HOME/.local/bin:$PATH"
source "$HOME/.cargo/env"
EOF
source ~/.bashrc
mkdir -p ~/.config/nabi ~/.cache/nabi ~/.local/share/nabi ~/.local/state/nabi ~/.local/bin

# 4. Build nabi (12 min)
cd ~/nabia/core
git clone <nabi-cli-repo>  # Or: cp -r /mnt/c/Users/<you>/Downloads/nabi-cli .
cd nabi-cli
make config
make build
make install

# 5. Verify (1 min)
nabi --version
nabi self doctor
```

---

## Success Check

```bash
which nabi                   # /home/<you>/.local/bin/nabi
nabi --version              # nabi 0.1.0
nabi self doctor            # ✅ All checks pass
rustc --version             # 1.88.0+
uv --version                # 0.8.22+
ls /mnt/c/Users/            # ✅ Windows access works
```

---

## Common Issues

| Issue | Fix |
|-------|-----|
| `nabi: command not found` | `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc` |
| Build fails "linker not found" | `sudo apt install -y build-essential` |
| Build very slow | **CRITICAL**: Use `~/nabia/` (Linux filesystem), NOT `/mnt/c/` (Windows filesystem) |
| WSL version is 1 | From PowerShell: `wsl --set-version Ubuntu-22.04 2` |

---

## WSL-Specific Notes

**Performance**:
```bash
# ✅ FAST - Build here
~/nabia/core/nabi-cli/

# ❌ SLOW - Never build here (5-10x slower!)
/mnt/c/projects/nabi-cli/
```

**Windows Interop**:
```bash
# Access Windows files
ls /mnt/c/Users/<WindowsUsername>/

# Access WSL from Windows
# In Explorer: \\wsl$\Ubuntu-22.04\home\<username>
```

**Resource Limits** (if needed):
```ini
# Create C:\Users\<WindowsUsername>\.wslconfig
[wsl2]
memory=4GB
processors=2
swap=2GB

# Then: wsl --shutdown
```

---

## What Works

✅ `nabi self doctor` - Health checks
✅ `nabi self config` - Configuration
✅ `nabi repo check` - Repository validation

⏳ `nabi claude/data/federation` - Commanders pending (future)

---

**Full Guide**: [DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md](DEPLOYMENT_PLAYBOOK_WSL2_UBUNTU.md)
**Troubleshooting**: [DEPLOYMENT_TROUBLESHOOTING.md](DEPLOYMENT_TROUBLESHOOTING.md)
