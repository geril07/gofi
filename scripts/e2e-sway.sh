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
MODE=""
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
                       [--mode apps|run|clipboard] [--app-id gofi]
                       [--evidence target/evidence/e2e]

Exit codes: 0 pass, 1 check failed, 2 usage/setup error.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; shift 2 ;;
    --app-id) APP_ID="${2:-}"; shift 2 ;;
    --query) QUERY="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-}"; shift 2 ;;
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
output HEADLESS-1 resolution 1920x1080
default_border none
for_window [app_id="$APP_ID"] floating enable, border none, resize set 50 ppt 495 px, move position center
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
mkdir -p "$RT/ready" "$RT/state"
APP_ARGS=()
[ -n "$MODE" ] && APP_ARGS+=("$MODE")
nested env RUNE_READY_DIR="$RT/ready" XDG_STATE_HOME="$RT/state" \
  "$BINARY" "${APP_ARGS[@]}" >"$EVIDENCE/app.log" 2>&1 &
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

read -r X Y W H < <(nested swaymsg -s "$SOCK" -t get_tree \
  | jq -r --arg id "$APP_ID" '.. | objects | select(.app_id? == $id) | "\(.rect.x) \(.rect.y) \(.rect.width) \(.rect.height)"' | head -1)
[ "$W" = "960" ] && [ "$H" = "495" ] || fail "expected 960x495, got ${W}x${H}"
[ "$X" = "480" ] || fail "expected x 480, got $X"
# 495 is odd, so the centred y is 292.5; Sway floors a toplevel to 292 while a
# layer-shell rounds rofi up to 293. Both are the same centred position.
case "$Y" in
  292|293) ;;
  *) fail "expected y 292 or 293, got $Y" ;;
esac

# Let the asynchronous icon decode finish before the evidence screenshot.
sleep 0.6
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

# Single instance: a second invocation closes the open window.
nested env RUNE_READY_DIR="$RT/ready2" XDG_STATE_HOME="$RT/state" \
  "$BINARY" "${APP_ARGS[@]}" >"$EVIDENCE/app2.log" 2>&1 &
APP_PID=$!
for _ in $(seq 1 100); do
  if tree_has_app; then break; fi
  sleep 0.1
done
tree_has_app || fail "the relaunch did not open a window"

nested env XDG_STATE_HOME="$RT/state" "$BINARY" "${APP_ARGS[@]}" \
  >"$EVIDENCE/second.log" 2>&1 || fail "the second invocation exited nonzero"

for _ in $(seq 1 100); do
  if ! tree_has_app; then break; fi
  sleep 0.1
done
tree_has_app && fail "the second invocation did not close the open launcher"

echo "PASS: app_id=$APP_ID focused, window opened"
[ -n "$QUERY" ] && echo "PASS: typed query and captured $EVIDENCE/typed.png"
echo "PASS: Escape closed the window"
echo "PASS: a second invocation closed the open launcher"
echo "evidence: $EVIDENCE"
