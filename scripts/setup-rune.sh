#!/usr/bin/env bash
# Prepare the patched Rune checkout the build uses.
#
# references/rune stays the pristine pinned clone. vendor/rune is a scratch
# checkout of the same revision with the local Rune patches in
# docs/research/rune-*.patch applied. The patch files are the durable copy;
# this script is idempotent.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PIN="04b38ae0a73bda03f010e97d2fff540ab91f67b0"
REF="$ROOT/references/rune"
VENDOR="$ROOT/vendor/rune"
CARGO="$ROOT/crates/gofi/Cargo.toml"

die() { echo "setup-rune: $*" >&2; exit 1; }

shopt -s nullglob
PATCHES=("$ROOT"/docs/research/rune-*.patch)
shopt -u nullglob
[ "${#PATCHES[@]}" -gt 0 ] || die "no Rune patches in docs/research"
[ -f "$CARGO" ] || die "missing $CARGO"
grep -q 'path = "../../vendor/rune/crates/rune"' "$CARGO" ||
  die "crates/gofi/Cargo.toml does not point at vendor/rune"

if [ ! -d "$REF/.git" ]; then
  echo "setup-rune: fetching Rune $PIN into references/rune"
  mkdir -p "$ROOT/references"
  git init --quiet "$REF"
  git -C "$REF" remote add origin https://github.com/anomalyco/rune
  git -C "$REF" fetch --quiet --depth 1 origin "$PIN" ||
    die "could not fetch Rune $PIN; check the network and rerun"
  git -C "$REF" checkout --quiet -B pinned FETCH_HEAD
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

for PATCH in "${PATCHES[@]}"; do
  # Applied already: the reverse check succeeds. Nothing to do.
  if git -C "$VENDOR" apply --reverse --check "$PATCH" >/dev/null 2>&1; then
    continue
  fi
  if git -C "$VENDOR" apply --check "$PATCH" >/dev/null 2>&1; then
    git -C "$VENDOR" apply "$PATCH"
    echo "setup-rune: applied $(basename "$PATCH") to vendor/rune"
    continue
  fi
  die "$(basename "$PATCH") does not apply cleanly to vendor/rune; remove vendor/rune and rerun"
done
