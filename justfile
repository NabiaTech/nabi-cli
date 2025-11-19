# Justfile for XDG-compliant nabi CLI builds
# Usage: just <target> or just --list for all targets

# Detect XDG paths - resolved via shell to handle env var defaults
# These are used in bash recipes where ${HOME} etc. will be available
# For justfile variables, we use safe defaults
home := env_var('HOME')
# Placeholder paths - actual values resolved in recipe with bash eval
cache_target_dir_base := 'nabi/nabi-cli/target'
nabi_data_bin := home + '/.local/share/nabi/bin'
zsh_completion_dir := home + '/.zsh/completions'
zsh_completion_file := zsh_completion_dir + '/_nabi'

# Code signing identity (override with NABI_SIGNING_IDENTITY env var)
# Default: auto-detect Apple Development certificate
signing_identity := env_var_or_default('NABI_SIGNING_IDENTITY', 'auto')

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

# Sign binary with Apple Developer certificate (or adhoc)
@sign:
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"

    if [ ! -f "${BINARY}" ]; then
        echo "❌ Binary not found: ${BINARY}"
        echo "   Run 'just build' first"
        exit 1
    fi

    # Determine signing identity
    if [ "{{signing_identity}}" = "auto" ]; then
        # Auto-detect Apple Development certificate
        IDENTITY=$(security find-identity -v -p codesigning | grep "Apple Development" | head -1 | awk -F'"' '{print $2}')
        if [ -z "$IDENTITY" ]; then
            echo "⚠️  No Apple Development certificate found, using adhoc signing"
            IDENTITY="-"
        else
            echo "🔐 Found certificate: $IDENTITY"
        fi
    elif [ "{{signing_identity}}" = "adhoc" ] || [ "{{signing_identity}}" = "-" ]; then
        IDENTITY="-"
        echo "🔐 Using adhoc signing"
    else
        IDENTITY="{{signing_identity}}"
        echo "🔐 Using identity: $IDENTITY"
    fi

    # Sign the binary
    codesign --force --deep --sign "$IDENTITY" "${BINARY}" 2>&1 | grep -v "replacing existing signature" || true

    # Clear quarantine attributes
    xattr -cr "${BINARY}" 2>/dev/null || true

    echo "✅ Binary signed successfully"

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
@install-quiet: completions sign
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    NABI_BIN="${XDG_DATA}/nabi/bin"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    mkdir -p "${NABI_BIN}"
    cp "${BINARY}" "${NABI_BIN}/nabi"
    chmod +x "${NABI_BIN}/nabi"

    # Re-sign the installed binary (preserve original signature)
    IDENTITY=$(security find-identity -v -p codesigning | grep 'Apple Development' | head -1 | awk -F'"' '{print $2}')
    if [ -n "$IDENTITY" ]; then
        codesign --force --deep --sign "$IDENTITY" "${NABI_BIN}/nabi" >/dev/null 2>&1
    else
        codesign --force --deep --sign - "${NABI_BIN}/nabi" >/dev/null 2>&1
    fi
    xattr -cr "${NABI_BIN}/nabi" 2>/dev/null || true

    echo "✅ Installed nabi to ${NABI_BIN}/nabi"

# Generate zsh completions from the freshly built binary with integrated dynamic enhancements
@completions: build
    #!/bin/bash
    set -e
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    ZSH_COMP_DIR="$HOME/.zsh/completions"
    ZSH_COMP_FILE="${ZSH_COMP_DIR}/_nabi"

    mkdir -p "${ZSH_COMP_DIR}" .build
    "${BINARY}" completions zsh > .build/_nabi_static
    
    # Post-process: Replace : or :_default with specific completion functions
    # This is the unified strategy - no overriding _nabi, just define specific functions
    echo "🔧 Post-processing completion file to add dynamic completion functions..."
    
    # 1. Replace pane argument for send-prompt (no completion function, just :)
    # Match: '::pane -- ...:' and add completion function
    sed -i '' "s/'::pane -- \(.*\):'/'::pane -- \1:_nabi__tmux__send_prompt_pane'/" .build/_nabi_static
    
    # 2. Replace service argument for port shift
    sed -i '' 's/:service -- Service name to migrate:_default/:service -- Service name to migrate:_nabi__port__shift_service/' .build/_nabi_static
    
    # 3 & 4. Replace tool arguments for both nabi exec and nabi tool exec
    # Use Python script to properly track tool section vs top-level exec
    python3 scripts/post-process-completions.py .build/_nabi_static
    
    # 5. Replace event_id argument for events ack
    sed -i '' 's/:event_id -- Event ID to acknowledge:_default/:event_id -- Event ID to acknowledge:_nabi__events__ack_event_id/' .build/_nabi_static
    
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
@install: completions sign
    #!/bin/bash
    set -e
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    XDG_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
    NABI_BIN="${XDG_DATA}/nabi/bin"
    BINARY="${XDG_CACHE}/nabi/nabi-cli/target/release/nabi"
    mkdir -p "${NABI_BIN}"
    cp "${BINARY}" "${NABI_BIN}/nabi"
    chmod +x "${NABI_BIN}/nabi"

    # Re-sign the installed binary with Apple Development certificate
    echo "🔐 Signing installed binary..."
    IDENTITY=$(security find-identity -v -p codesigning | grep 'Apple Development' | head -1 | awk -F'"' '{print $2}')
    if [ -n "$IDENTITY" ]; then
        codesign --force --deep --sign "$IDENTITY" "${NABI_BIN}/nabi"
        echo "   ✓ Signed with: $IDENTITY"
    else
        echo "   ⚠️  No Apple Development certificate found, using adhoc"
        codesign --force --deep --sign - "${NABI_BIN}/nabi"
    fi
    xattr -cr "${NABI_BIN}/nabi" 2>/dev/null || true

    # Verify the binary actually works
    echo "🧪 Verifying installation..."
    if "${NABI_BIN}/nabi" --version >/dev/null 2>&1; then
        echo "   ✓ Binary executes successfully"
    else
        EXIT_CODE=$?
        echo "   ❌ Binary verification failed with exit code: $EXIT_CODE"
        if [ $EXIT_CODE -eq 137 ]; then
            echo "   💡 Exit 137 = SIGKILL. Re-trying with adhoc signing..."
            codesign --force --deep --sign - "${NABI_BIN}/nabi"
            xattr -cr "${NABI_BIN}/nabi" 2>/dev/null || true
            if "${NABI_BIN}/nabi" --version >/dev/null 2>&1; then
                echo "   ✓ Recovery successful with adhoc signing"
            else
                echo "   ❌ Recovery failed. Please report this issue."
                exit 1
            fi
        else
            exit 1
        fi
    fi

    echo "✅ Installed nabi to ${NABI_BIN}/nabi"
    echo ""
    "${NABI_BIN}/nabi" --version

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

# Health check: verify nabi is properly signed and working
@health:
    #!/bin/bash
    set -e
    XDG_DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    NABI_BIN="${XDG_DATA}/nabi/bin/nabi"

    echo "🏥 nabi Health Check"
    echo "===================="
    echo ""

    # Check if binary exists
    if [ ! -f "${NABI_BIN}" ]; then
        echo "❌ Binary not found at: ${NABI_BIN}"
        echo "   Run 'just install' to install nabi"
        exit 1
    fi
    echo "✓ Binary exists: ${NABI_BIN}"

    # Check if binary is executable
    if [ ! -x "${NABI_BIN}" ]; then
        echo "❌ Binary is not executable"
        echo "   Run: chmod +x ${NABI_BIN}"
        exit 1
    fi
    echo "✓ Binary is executable"

    # Check code signature
    if codesign -dv "${NABI_BIN}" >/dev/null 2>&1; then
        AUTHORITY=$(codesign -dv --verbose=2 "${NABI_BIN}" 2>&1 | grep "^Authority=" | head -1 | cut -d= -f2)
        if [ -n "$AUTHORITY" ]; then
            echo "✓ Code signed: $AUTHORITY"
        else
            echo "✓ Code signed (adhoc)"
        fi
    else
        echo "❌ Binary is not properly signed"
        echo "   Run 'just install' to re-sign"
        exit 1
    fi

    # Check for quarantine attributes
    if xattr "${NABI_BIN}" 2>/dev/null | grep -q "com.apple.quarantine"; then
        echo "⚠️  Quarantine attributes present (may cause issues)"
        echo "   Run: xattr -cr ${NABI_BIN}"
    else
        echo "✓ No quarantine attributes"
    fi

    # Test execution
    if "${NABI_BIN}" --version >/dev/null 2>&1; then
        VERSION=$("${NABI_BIN}" --version)
        echo "✓ Binary executes successfully: $VERSION"
    else
        EXIT_CODE=$?
        echo "❌ Binary execution failed with exit code: $EXIT_CODE"
        if [ $EXIT_CODE -eq 137 ]; then
            echo "   💡 Exit 137 = SIGKILL (killed by macOS)"
            echo "   Run 'just install' to fix signing"
        fi
        exit 1
    fi

    # Check if it's in PATH
    if command -v nabi >/dev/null 2>&1; then
        WHICH_NABI=$(which nabi)
        if [ "$WHICH_NABI" = "${NABI_BIN}" ] || [ "$WHICH_NABI" = "$HOME/.local/bin/nabi" ]; then
            echo "✓ nabi is in PATH: $WHICH_NABI"
        else
            echo "⚠️  Different nabi in PATH: $WHICH_NABI"
            echo "   Expected: ${NABI_BIN}"
        fi
    else
        echo "⚠️  nabi not in PATH"
        echo "   Add to PATH: export PATH=\"$HOME/.local/share/nabi/bin:\$PATH\""
    fi

    echo ""
    echo "🎉 All checks passed! nabi is healthy."
