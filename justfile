# Justfile for XDG-compliant nabi CLI builds
# Usage: just <target> or just --list for all targets

# Detect XDG paths - resolved via shell to handle env var defaults
# These are used in bash recipes where ${HOME} etc. will be available
# For justfile variables, we use safe defaults
home := env_var('HOME')
# Placeholder paths - actual values resolved in recipe with bash eval
cache_target_dir_base := 'nabi/nabi-cli/target'
nabi_data_bin := home + '/.local/share/nabi/bin'
zsh_completion_dir := home + '/.cache/zsh/completions'
zsh_completion_file := zsh_completion_dir + '/_nabi'

# Default target (show help)
default:
    @just --list

# Generate XDG-compliant cargo config
@config:
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    mkdir -p .cargo
    sed "s|\\\$XDG_CACHE_HOME|${XDG_CACHE}|g" .cargo/config.toml.template > .cargo/config.toml
    echo "Generated .cargo/config.toml with target-dir: ${XDG_CACHE}/nabi/nabi-cli/target"

# Build with XDG paths
@build: config
    cargo build --release

# Watch mode: rebuild and install on file changes
@watch:
    #!/bin/bash
    if command -v cargo-watch >/dev/null 2>&1; then
        echo "🔍 Watching for changes (cargo-watch)..."
        cargo watch -x "build --release" -s "just install-quiet"
    elif command -v fswatch >/dev/null 2>&1; then
        echo "🔍 Watching for changes (fswatch)..."
        fswatch -o src/ Cargo.toml | xargs -n1 -I{} just build install-quiet
    elif command -v entr >/dev/null 2>&1; then
        echo "🔍 Watching for changes (entr)..."
        find src/ Cargo.toml -type f | entr -c just build install-quiet
    else
        echo "❌ No file watcher found. Install one of:"
        echo "   - cargo-watch: cargo install cargo-watch"
        echo "   - fswatch: brew install fswatch"
        echo "   - entr: brew install entr"
        exit 1
    fi

# Dev mode: build, install, then watch
@dev: build install
    echo "✅ Initial build complete. Starting watch mode..."
    just watch

# Install without verbose output (for watch mode)
@install-quiet: completions
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    NABI_BIN="${XDG_DATA}/nabi/bin"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    mkdir -p "${NABI_BIN}"
    cp "${BINARY}" "${NABI_BIN}/nabi"
    chmod +x "${NABI_BIN}/nabi"
    echo "✅ Installed nabi to ${NABI_BIN}/nabi"

# Generate zsh completions from the freshly built binary with integrated dynamic enhancements
@completions: build
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    ZSH_COMP_DIR="${XDG_CACHE}/zsh/completions"
    ZSH_COMP_FILE="${ZSH_COMP_DIR}/_nabi"

    mkdir -p "${ZSH_COMP_DIR}" .build
    "${BINARY}" completions zsh > .build/_nabi_static
    cp .build/_nabi_static "${ZSH_COMP_FILE}"
    echo '' >> "${ZSH_COMP_FILE}"
    echo '# Dynamic tmux completion enhancements (injected at build time)' >> "${ZSH_COMP_FILE}"
    tail -n +2 contrib/nabi-completions-dynamic.zsh >> "${ZSH_COMP_FILE}"
    cp contrib/nabi-completions-dynamic.zsh "${ZSH_COMP_DIR}/nabi-completions-dynamic.zsh"
    echo "✓ Generated integrated zsh completion: ${ZSH_COMP_FILE}"
    echo "🔍 Validating completion composition..."
    bash scripts/validate-completions.sh --verbose || { echo "❌ Completion validation failed!"; exit 1; }
    echo "✅ Completion composition validated"

# Install to ~/.local/share/nabi/bin and refresh completions
@install: completions
    #!/bin/bash
    set -e
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    NABI_BIN="${XDG_DATA}/nabi/bin"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    mkdir -p "${NABI_BIN}"
    cp "${BINARY}" "${NABI_BIN}/nabi"
    chmod +x "${NABI_BIN}/nabi"
    echo "✅ Installed nabi to ${NABI_BIN}/nabi"

# Clean build artifacts
@clean:
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    cargo clean
    rm -rf "${XDG_CACHE}/nabi/nabi-cli/target"

# Run tests
@test:
    cargo test

# Quick rebuild and install
@quick: build install
    echo "Quick build and install complete"

# Release-ready binary and completion artifacts
@release: install
    echo "Release-ready binary and completions are up to date"
