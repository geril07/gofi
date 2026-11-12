#!/usr/bin/env bash
# Prepare the patched Rune checkout the build uses.
#
# references/rune stays the pristine pinned clone. vendor/rune is a scratch
# checkout of the same revision with docs/research/rune-startup.patch applied.
# The patch file is the durable copy; this script is idempotent.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PIN="04b38ae0a73bda03f010e97d2fff540ab91f67b0"
REF="$ROOT/references/rune"
VENDOR="$ROOT/vendor/rune"
PATCH="$ROOT/docs/research/rune-startup.patch"
CARGO="$ROOT/crates/gofi/Cargo.toml"

die() { echo "setup-rune: $*" >&2; exit 1; }

[ -f "$PATCH" ] || die "missing $PATCH"
[ -f "$CARGO" ] || die "missing $CARGO"
grep -q 'path = "../../vendor/rune/crates/rune"' "$CARGO" ||
  die "crates/gofi/Cargo.toml does not point at vendor/rune"

if [ ! -d "$REF/.git" ]; then
  die "missing references/rune; clone it first:
  git clone https://github.com/anomalyco/rune references/rune
  git -C references/rune checkout $PIN"
fi
HEAD=$(git -C "$REF" rev-parse HEAD)
[ "$HEAD" = "$PIN" ] || die "references/rune is at $HEAD, expected $PIN"

if [ -e "$VENDOR" ] && [ ! -d "$VENDOR/.git" ]; then
  die "vendor/rune exists but is not a git checkout; remove it and rerun"
fi
if [ ! -d "$VENDOR/.git" ]; then
  mkdir -p "$ROOT/vendor"
  git clone --quiet "$REF" "$VENDOR"
fi

HEAD=$(git -C "$VENDOR" rev-parse HEAD)
[ "$HEAD" = "$PIN" ] ||
  die "vendor/rune is at $HEAD, expected $PIN; remove vendor/rune and rerun"

# Applied already: the reverse check succeeds. Nothing to do.
if git -C "$VENDOR" apply --reverse --check "$PATCH" >/dev/null 2>&1; then
  exit 0
fi
if git -C "$VENDOR" apply --check "$PATCH" >/dev/null 2>&1; then
  git -C "$VENDOR" apply "$PATCH"
  echo "setup-rune: applied $(basename "$PATCH") to vendor/rune"
  exit 0
fi
die "the startup patch does not apply cleanly to vendor/rune; remove vendor/rune and rerun"
