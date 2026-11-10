#!/usr/bin/env bash
# The project pins Rune to one revision. The dependency is a path into the
# read-only clone at references/rune, so a wrong checkout would silently build
# against a different API.
set -euo pipefail

PIN="04b38ae0a73bda03f010e97d2fff540ab91f67b0"
RUNES="${1:-references/rune}"

if [ ! -d "$RUNES/.git" ]; then
  echo "check-rune: missing $RUNES" >&2
  echo "  git clone https://github.com/anomalyco/rune $RUNES" >&2
  echo "  git -C $RUNES checkout $PIN" >&2
  exit 1
fi

HEAD=$(git -C "$RUNES" rev-parse HEAD)
if [ "$HEAD" != "$PIN" ]; then
  echo "check-rune: Rune is at $HEAD" >&2
  echo "  expected $PIN (git -C $RUNES checkout $PIN)" >&2
  exit 1
fi
