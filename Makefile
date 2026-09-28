.PHONY: run install bundle test check fmt clean

# Honour CARGO_TARGET_DIR / .cargo/config.toml rather than assuming ./target.
TARGET_DIR := $(shell cargo metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
BIN_DIR ?= $(HOME)/.local/bin

# Build the bundle and launch it (kills any running copy first).
run: bundle
	@pkill -x wallbar 2>/dev/null || true
	open "$(TARGET_DIR)/Wallbar.app"

# Put the app in /Applications, link the CLI into ~/.local/bin, and run it.
# Start-at-login registers whatever path the app was launched from, so the
# copy in /Applications is the one that gets launched.
install: bundle
	@pkill -x wallbar 2>/dev/null || true
	rm -rf "/Applications/Wallbar.app"
	cp -R "$(TARGET_DIR)/Wallbar.app" "/Applications/Wallbar.app"
	mkdir -p "$(BIN_DIR)"
	ln -sf "/Applications/Wallbar.app/Contents/MacOS/wallbar" "$(BIN_DIR)/wallbar"
	open "/Applications/Wallbar.app"

bundle:
	./scripts/bundle.sh

test:
	cargo test --workspace

check:
	cargo fmt --all --check
	cargo clippy --all-targets --workspace -- -D warnings

fmt:
	cargo fmt --all

clean:
	cargo clean
