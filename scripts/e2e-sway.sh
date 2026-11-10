#!/usr/bin/env bash
# Nested headless Sway smoke test for a launcher window.
#
# Never touches the live session: it starts its own Sway with a private
# XDG_RUNTIME_DIR, unsets SWAYSOCK/I3SOCK, and asserts the IPC socket is inside
# the temp dir.
#
# Usage:
#   scripts/e2e-sway.sh --binary target/release/gofi [--query code]
#                        [--app-id gofi] [--evidence target/evidence/e2e]
#
# Exit codes: 0 pass, 1 check failed, 2 usage/setup error.

set -euo pipefail

BINARY=""
APP_ID="gofi"
QUERY=""
EVIDENCE="target/evidence/e2e"
WINDOW_TIMEOUT_MS=10000

usage() {
  cat <<'EOF'
Nested headless Sway smoke test for a launcher window.

Never touches the live session: it starts its own Sway with a private
XDG_RUNTIME_DIR, unsets SWAYSOCK/I3SOCK, and asserts the IPC socket is inside
the temp dir.

Usage:
  scripts/e2e-sway.sh --binary target/release/gofi [--query code]
                       [--app-id gofi] [--evidence target/evidence/e2e]

Exit codes: 0 pass, 1 check failed, 2 usage/setup error.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; shift 2 ;;
    --app-id) APP_ID="${2:-}"; shift 2 ;;
    --query) QUERY="${2:-}"; shift 2 ;;
    --evidence) EVIDENCE="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[ -n "$BINARY" ] || { echo "--binary is required" >&2; exit 2; }
[ -x "$BINARY" ] || { echo "not executable: $BINARY" >&2; exit 2; }
BINARY=$(realpath "$BINARY")

mkdir -p "$EVIDENCE"
EVIDENCE=$(realpath "$EVIDENCE")

RT=$(mktemp -d /tmp/opencode/gofi-e2e.XXXXXX)
chmod 700 "$RT"

SWAY_PID=""
APP_PID=""
KEEPER_PID=""

cleanup() {
  for pid in "$APP_PID" "$KEEPER_PID" "$SWAY_PID"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
  rm -rf "$RT"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

cat > "$RT/config" <<EOF
output HEADLESS-1 resolution 1280x800
default_border none
for_window [app_id="$APP_ID"] floating enable, border none, move position center
for_window [app_id="$APP_ID"] resize set 960 495
EOF

# The nested compositor. No live-session variables survive.
env -u SWAYSOCK -u I3SOCK \
  XDG_RUNTIME_DIR="$RT" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 sway -c "$RT/config" \
  >"$EVIDENCE/sway.log" 2>&1 &
SWAY_PID=$!

SOCK=""
for _ in $(seq 1 100); do
  SOCK=$(find "$RT" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit)
  [ -n "$SOCK" ] && break
  sleep 0.1
done
[ -n "$SOCK" ] || fail "nested Sway did not start (see $EVIDENCE/sway.log)"
case "$SOCK" in
  "$RT"/*) ;;
  *) fail "refusing foreign Sway socket: $SOCK" ;;
esac

WL_DISPLAY=""
for _ in $(seq 1 100); do
  WL_SOCK=$(find "$RT" -maxdepth 1 -type s -name 'wayland-*' -print -quit)
  [ -n "$WL_SOCK" ] && WL_DISPLAY=$(basename "$WL_SOCK") && break
  sleep 0.1
done
[ -n "$WL_DISPLAY" ] || fail "nested Wayland display did not appear"

nested() {
  env -u SWAYSOCK -u I3SOCK \
    XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY="$WL_DISPLAY" "$@"
}

# Start the app with a readiness file so cold start is recorded for free.
mkdir -p "$RT/ready"
nested env RUNE_READY_DIR="$RT/ready" "$BINARY" >"$EVIDENCE/app.log" 2>&1 &
APP_PID=$!

tree_has_app() {
  nested swaymsg -s "$SOCK" -t get_tree 2>/dev/null \
    | jq -e --arg id "$APP_ID" '.. | objects | select(.app_id? == $id)' >/dev/null
}

for _ in $(seq 1 $((WINDOW_TIMEOUT_MS / 100))); do
  if tree_has_app; then break; fi
  sleep 0.1
done
tree_has_app || fail "no window with app_id=$APP_ID (see $EVIDENCE/app.log)"

nested swaymsg -s "$SOCK" -t get_tree >"$EVIDENCE/tree.json"
READY=$(find "$RT/ready" -maxdepth 1 -type f -print -quit || true)
[ -n "$READY" ] && cp "$READY" "$EVIDENCE/ready.json" || true

FOCUSED=$(nested swaymsg -s "$SOCK" -t get_tree \
  | jq -r --arg id "$APP_ID" '.. | objects | select(.app_id? == $id) | .focused' | head -1)
[ "$FOCUSED" = "true" ] || fail "window app_id=$APP_ID is not focused"

nested grim "$EVIDENCE/opened.png"

# In nested headless Sway, wtype owns the only keyboard. When it exits, the seat
# loses keyboard capability and a launcher that closes on focus loss exits. Keep a
# virtual keyboard alive for the rest of the test.
nested wtype -d 60000 -k Shift_L -k Shift_L >"$EVIDENCE/keeper.log" 2>&1 &
KEEPER_PID=$!
sleep 0.3

if [ -n "$QUERY" ]; then
  nested wtype -d 80 "$QUERY"
  sleep 0.4
  kill -0 "$APP_PID" 2>/dev/null || fail "app exited while typing the query"
  nested grim "$EVIDENCE/typed.png"
fi

nested wtype -k Escape
sleep 0.4

if tree_has_app; then
  fail "window still present after Escape"
fi

echo "PASS: app_id=$APP_ID focused, window opened"
[ -n "$QUERY" ] && echo "PASS: typed query and captured $EVIDENCE/typed.png"
echo "PASS: Escape closed the window"
echo "evidence: $EVIDENCE"
