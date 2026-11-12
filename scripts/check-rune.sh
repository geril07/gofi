#!/usr/bin/env bash
# Verify the Rune checkout the build uses: vendor/rune at the pinned revision
# with the local Rune patches in docs/research/rune-*.patch applied.
# references/rune is the pristine source clone; both must be at the pin.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PIN="04b38ae0a73bda03f010e97d2fff540ab91f67b0"
REF="$ROOT/references/rune"
VENDOR="$ROOT/vendor/rune"
CARGO="$ROOT/crates/gofi/Cargo.toml"

die() {
  echo "check-rune: $*" >&2
  echo "  run scripts/setup-rune.sh" >&2
  exit 1
}

shopt -s nullglob
PATCHES=("$ROOT"/docs/research/rune-*.patch)
shopt -u nullglob
[ "${#PATCHES[@]}" -gt 0 ] || die "no Rune patches in docs/research"
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

for PATCH in "${PATCHES[@]}"; do
  git -C "$VENDOR" apply --reverse --check "$PATCH" >/dev/null 2>&1 ||
    die "$(basename "$PATCH") is not applied to vendor/rune"
done
