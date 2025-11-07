# nabi CLI Development Workflow

## Quick Start

### Standard Build & Install
```bash
make build      # Build release binary
make install    # Install to ~/.local/bin/nabi
make quick      # Build + install in one command
```

### Development Mode (Auto-rebuild on changes)
```bash
make dev        # Initial build + install, then watch for changes
```

This will:
1. Build the release binary
2. Install to `~/.local/bin/nabi`
3. Watch `src/` and `Cargo.toml` for changes
4. Auto-rebuild and reinstall on file changes

## File Watchers

The `make watch` command supports multiple file watchers (in order of preference):

1. **cargo-watch** (Recommended for Rust)
   ```bash
   cargo install cargo-watch
   ```

2. **fswatch** (macOS/Linux)
   ```bash
   brew install fswatch  # macOS
   ```

3. **entr** (Cross-platform)
   ```bash
   brew install entr  # macOS
   apt install entr   # Linux
   ```

## CI/CD Workflows

### Option 1: Manual Build & Install
```bash
# After making changes
make build install
```

### Option 2: Watch Mode (Recommended for active development)
```bash
# Start watch mode in a terminal
make dev

# In another terminal, edit files
# Changes auto-rebuild and reinstall
```

### Option 3: Git Hook (Auto-build on commit)
```bash
# Enable post-commit hook
chmod +x .git/hooks/post-commit

# Now every commit auto-builds and installs
git commit -m "Update feature"
# → Auto-builds and installs after commit
```

### Option 4: Pre-commit Hook (Build before commit)
```bash
# Create .git/hooks/pre-commit
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
cd "$(git rev-parse --show-toplevel)/core/nabi-cli" || exit 0
make test && make build
EOF
chmod +x .git/hooks/pre-commit
```

## Build Artifacts

- **Source**: `src/` directory
- **Build output**: `~/.cache/nabi/nabi-cli/target/release/nabi` (XDG cache)
- **Installed binary**: `~/.local/bin/nabi`
- **Completions**: `~/.cache/zsh/completions/_nabi`

## Testing

```bash
make test              # Run Rust tests
cargo test --release   # Run tests with release profile
```

## Cleanup

```bash
make clean             # Remove build artifacts
```

## Troubleshooting

### Binary not updating after install
```bash
# Check which binary is being used
which nabi

# Verify installed binary timestamp
ls -lh ~/.local/bin/nabi

# Force rebuild and reinstall
make clean build install
```

### Watch mode not working
```bash
# Check if watcher is installed
which cargo-watch || which fswatch || which entr

# Install cargo-watch (recommended)
cargo install cargo-watch
```

### Build fails
```bash
# Clean and rebuild
make clean
make build

# Check Rust version (requires 1.70+)
rustc --version
```
