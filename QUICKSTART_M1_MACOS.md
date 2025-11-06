# Quick Start: M1 MacBook Pro (30-Second Overview)

**Time**: 45-60 minutes | **Platform**: M1/M2/M3 macOS Sonoma/Ventura | **Difficulty**: Intermediate

---

## Prerequisites (2 minutes)

```bash
# Check you have:
sw_vers                      # macOS 13.x or 14.x
uname -m                     # arm64
df -h ~                      # 5GB+ free
```

---

## Installation (5 Commands)

```bash
# 1. Install Rust (5 min)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 2. Install Python/uv (5 min)
curl -LsSf https://astral.sh/uv/install.sh | sh
uv python install 3.13

# 3. Set up XDG (1 min)
cat >> ~/.zshenv << 'EOF'
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export PATH="$HOME/.local/bin:$PATH"
source "$HOME/.cargo/env"
EOF
source ~/.zshenv
mkdir -p ~/.config/nabi ~/.cache/nabi ~/.local/share/nabi ~/.local/state/nabi ~/.local/bin

# 4. Build nabi (15 min)
cd ~/nabia/core
git clone <nabi-cli-repo>  # Or copy existing source
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
which nabi                   # /Users/<you>/.local/bin/nabi
nabi --version              # nabi 0.1.0
nabi self doctor            # ✅ All checks pass
rustc --version             # 1.88.0+
uv --version                # 0.8.22+
```

---

## Common Issues

| Issue | Fix |
|-------|-----|
| `nabi: command not found` | `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshenv && source ~/.zshenv` |
| Build fails "linker not found" | `xcode-select --install` |
| Build very slow | Check `uname -m` shows `arm64` (not running under Rosetta) |

---

## What Works

✅ `nabi self doctor` - Health checks
✅ `nabi self config` - Configuration
✅ `nabi repo check` - Repository validation

⏳ `nabi claude/data/federation` - Commanders pending (future)

---

**Full Guide**: [DEPLOYMENT_PLAYBOOK_M1_MACOS.md](DEPLOYMENT_PLAYBOOK_M1_MACOS.md)
**Troubleshooting**: [DEPLOYMENT_TROUBLESHOOTING.md](DEPLOYMENT_TROUBLESHOOTING.md)
