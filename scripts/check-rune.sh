#!/usr/bin/env bash
# Verify the Rune checkout the build uses: vendor/rune at the pinned revision
# with docs/research/rune-startup.patch applied. references/rune is the pristine
# source clone; both must be at the pin.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PIN="04b38ae0a73bda03f010e97d2fff540ab91f67b0"
REF="$ROOT/references/rune"
VENDOR="$ROOT/vendor/rune"
PATCH="$ROOT/docs/research/rune-startup.patch"
CARGO="$ROOT/crates/gofi/Cargo.toml"

die() {
  echo "check-rune: $*" >&2
  echo "  run scripts/setup-rune.sh" >&2
  exit 1
}

[ -f "$PATCH" ] || die "missing $PATCH"
[ -f "$CARGO" ] || die "missing $CARGO"
grep -q 'path = "../../vendor/rune/crates/rune"' "$CARGO" ||
  die "crates/gofi/Cargo.toml does not point at vendor/rune"

for pair in "references/rune:$REF" "vendor/rune:$VENDOR"; do
  name=${pair%%:*}
  dir=${pair#*:}
  [ -d "$dir/.git" ] || die "missing $name"
  HEAD=$(git -C "$dir" rev-parse HEAD)
  [ "$HEAD" = "$PIN" ] || die "$name is at $HEAD, expected $PIN"
done

git -C "$VENDOR" apply --reverse --check "$PATCH" >/dev/null 2>&1 ||
  die "the startup patch is not applied to vendor/rune"
