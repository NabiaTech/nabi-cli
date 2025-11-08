# Makefile for XDG-compliant nabi CLI builds

# Detect XDG paths
XDG_CACHE_HOME ?= $(HOME)/.cache
CACHE_TARGET_DIR = $(XDG_CACHE_HOME)/nabi/nabi-cli/target
ZSH_COMPLETION_DIR ?= $(XDG_CACHE_HOME)/zsh/completions
ZSH_COMPLETION_FILE = $(ZSH_COMPLETION_DIR)/_nabi

.PHONY: config build install clean test quick completions release watch dev

# Generate XDG-compliant cargo config
config:
	@mkdir -p .cargo
	@sed "s|\$$XDG_CACHE_HOME|$(XDG_CACHE_HOME)|g" .cargo/config.toml.template > .cargo/config.toml
	@echo "Generated .cargo/config.toml with target-dir: $(CACHE_TARGET_DIR)"

# Build with XDG paths
build: config
	cargo build --release

# Watch mode: rebuild and install on file changes
# Uses cargo-watch if available, otherwise falls back to fswatch/entr
watch:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		echo "🔍 Watching for changes (cargo-watch)..."; \
		cargo watch -x "build --release" -s "make install-quiet"; \
	elif command -v fswatch >/dev/null 2>&1; then \
		echo "🔍 Watching for changes (fswatch)..."; \
		fswatch -o src/ Cargo.toml | xargs -n1 -I{} make build install-quiet; \
	elif command -v entr >/dev/null 2>&1; then \
		echo "🔍 Watching for changes (entr)..."; \
		find src/ Cargo.toml -type f | entr -c make build install-quiet; \
	else \
		echo "❌ No file watcher found. Install one of:"; \
		echo "   - cargo-watch: cargo install cargo-watch"; \
		echo "   - fswatch: brew install fswatch"; \
		echo "   - entr: brew install entr"; \
		exit 1; \
	fi

# Dev mode: build, install, then watch
dev: build install
	@echo "✅ Initial build complete. Starting watch mode..."
	@$(MAKE) watch

# Install without verbose output (for watch mode)
install-quiet: completions
	@mkdir -p $(HOME)/.local/bin
	@cp $(CACHE_TARGET_DIR)/release/nabi $(HOME)/.local/bin/nabi
	@chmod +x $(HOME)/.local/bin/nabi
	@echo "✅ Installed nabi to $(HOME)/.local/bin/nabi"

# Generate zsh completions from the freshly built binary with integrated dynamic enhancements
# This target:
# 1. Generates static completion from clap
# 2. Injects dynamic tmux enhancements
# 3. Validates the composition against schema
# 4. Aborts build on validation failure
completions: build
	@mkdir -p $(ZSH_COMPLETION_DIR)
	@mkdir -p .build

	# Generate static completion from clap
	$(CACHE_TARGET_DIR)/release/nabi completions zsh > .build/_nabi_static

	# Create integrated completion: static + dynamic enhancements injected
	@cp .build/_nabi_static $(ZSH_COMPLETION_FILE)
	@echo '' >> $(ZSH_COMPLETION_FILE)
	@echo '# Dynamic tmux completion enhancements (injected at build time)' >> $(ZSH_COMPLETION_FILE)
	@tail -n +2 contrib/nabi-completions-dynamic.zsh >> $(ZSH_COMPLETION_FILE)

	# Preserve reference copy for debugging/maintenance
	@cp contrib/nabi-completions-dynamic.zsh $(ZSH_COMPLETION_DIR)/nabi-completions-dynamic.zsh
	@echo "✓ Generated integrated zsh completion: $(ZSH_COMPLETION_FILE)"

	# Validate the composition against schema (fail build on validation failure)
	@echo "🔍 Validating completion composition..."
	@bash scripts/validate-completions.sh --verbose || (echo "❌ Completion validation failed!"; exit 1)
	@echo "✅ Completion composition validated"

# Install to ~/.local/bin and refresh completions
install: completions
	@mkdir -p $(HOME)/.local/bin
	cp $(CACHE_TARGET_DIR)/release/nabi $(HOME)/.local/bin/nabi
	chmod +x $(HOME)/.local/bin/nabi
	@echo "Installed nabi to $(HOME)/.local/bin/nabi"

# Clean build artifacts
clean:
	cargo clean
	rm -rf $(CACHE_TARGET_DIR)

# Run tests
test:
	cargo test

# Quick rebuild and install
quick: build install
	@echo "Quick build and install complete"

# Release-ready binary and completion artifacts
release: install
	@echo "Release-ready binary and completions are up to date"
