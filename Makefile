# Makefile for XDG-compliant nabi CLI builds

# Detect XDG paths
XDG_CACHE_HOME ?= $(HOME)/.cache
CACHE_TARGET_DIR = $(XDG_CACHE_HOME)/nabi/nabi-cli/target

.PHONY: config build install clean test

# Generate XDG-compliant cargo config
config:
	@mkdir -p .cargo
	@sed "s|\$$XDG_CACHE_HOME|$(XDG_CACHE_HOME)|g" .cargo/config.toml.template > .cargo/config.toml
	@echo "Generated .cargo/config.toml with target-dir: $(CACHE_TARGET_DIR)"

# Build with XDG paths
build: config
	cargo build --release

# Install to ~/.local/bin
install: build
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
