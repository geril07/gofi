default: check

check-rune:
    scripts/check-rune.sh

check: check-rune fmt clippy test

fmt:
    cargo fmt --package gofi-core --package gofi --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

build: check-rune
    cargo build --release -p gofi

# Install the release binary into ~/.local/bin (override with BIN_DIR=...).
install: build
    install -Dm755 target/release/gofi "$${BIN_DIR:-$$HOME/.local/bin}/gofi"

# Open the launcher (apps mode).
run:
    cargo run -p gofi

# Open one mode: apps, run or clipboard.
run-mode mode:
    cargo run -p gofi -- {{mode}}

# just dump [mode] [query]
dump mode="apps" query="":
    cargo run -q -p gofi -- --dump --mode {{mode}} --query "{{query}}"

# Nested headless Sway: window geometry, focus, typing, Escape.
e2e binary="target/release/gofi" query="fire" mode="apps":
    scripts/e2e-sway.sh --binary {{binary}} --query "{{query}}" --mode "{{mode}}"

# Nested headless Sway: launch a .desktop app, copy a cliphist entry.
e2e-launch binary="target/release/gofi":
    scripts/e2e-launch.sh --binary {{binary}}

# Process start -> first on_ready under nested Sway.
measure mode="apps" runs="20":
    scripts/measure-startup.sh --binary target/release/gofi --mode {{mode}} --runs {{runs}}
