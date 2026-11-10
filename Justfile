default: check

check: fmt clippy test

fmt:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

build:
    cargo build --release -p gofi

# just dump [mode] [query]
dump mode="apps" query="":
    cargo run -q -p gofi -- --dump --mode {{mode}} --query "{{query}}"

# Nested headless Sway smoke test. The Rune UI arrives in M2; until then this
# needs a window-capable binary.
e2e binary="target/release/gofi" query="":
    scripts/e2e-sway.sh --binary {{binary}} --query "{{query}}"
