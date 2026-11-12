#!/usr/bin/env bash
# Nested headless Sway e2e for the two actions: launch an app and copy a
# cliphist entry. Never touches the live session or the real clipboard.
#
# Usage:
#   scripts/e2e-launch.sh --binary target/release/gofi [--evidence DIR]
#
# Exit codes: 0 pass, 1 check failed, 2 usage/setup error.

set -euo pipefail

BINARY=""
EVIDENCE="target/evidence/e2e-launch"

usage() {
  cat <<'EOF'
Usage:
  scripts/e2e-launch.sh --binary target/release/gofi [--evidence DIR]

Starts two private nested Sway sessions: one launches a fake .desktop app and
checks the frecency state, the other copies from a private cliphist database.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; shift 2 ;;
    --evidence) EVIDENCE="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[ -n "$BINARY" ] || { echo "--binary is required" >&2; exit 2; }
[ -x "$BINARY" ] || { echo "not executable: $BINARY" >&2; exit 2; }
BINARY=$(realpath "$BINARY")
CLIPHIST=$(command -v cliphist) || { echo "cliphist not found" >&2; exit 2; }

mkdir -p "$EVIDENCE"
EVIDENCE=$(realpath "$EVIDENCE")

RT=$(mktemp -d /tmp/opencode/gofi-e2e-launch.XXXXXX)
chmod 700 "$RT"

PIDS=()
cleanup() {
  for pid in "${PIDS[@]:-}"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
  rm -rf "$RT"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

cat > "$RT/config" <<'EOF'
output HEADLESS-1 resolution 1920x1080
default_border none
for_window [app_id="gofi"] floating enable, border none, resize set 50 ppt 495 px, move absolute position center
EOF

env -u SWAYSOCK -u I3SOCK \
  XDG_RUNTIME_DIR="$RT" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 sway -c "$RT/config" >"$EVIDENCE/sway.log" 2>&1 &
PIDS+=($!)

SOCK=""
for _ in $(seq 1 100); do
  SOCK=$(find "$RT" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit)
  [ -n "$SOCK" ] && break
  sleep 0.1
done
[ -n "$SOCK" ] || fail "nested Sway did not start"
case "$SOCK" in
  "$RT"/*) ;;
  *) fail "refusing foreign Sway socket: $SOCK" ;;
esac

WL_DISPLAY=""
for _ in $(seq 1 100); do
  WL_SOCK=$(find "$RT" -maxdepth 1 -type s -name 'wayland-*' -print -quit)
  if [ -n "$WL_SOCK" ]; then WL_DISPLAY=$(basename "$WL_SOCK"); break; fi
  sleep 0.1
done
[ -n "$WL_DISPLAY" ] || fail "nested Wayland display did not appear"

nested() {
  env -u SWAYSOCK -u I3SOCK \
    XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY="$WL_DISPLAY" "$@"
}

tree_has_app() {
  nested swaymsg -s "$SOCK" -t get_tree 2>/dev/null \
    | jq -e '.. | objects | select(.app_id? == "gofi")' >/dev/null
}

wait_for_window() {
  for _ in $(seq 1 100); do
    if tree_has_app; then return 0; fi
    sleep 0.1
  done
  return 1
}

wait_for_close() {
  for _ in $(seq 1 100); do
    if ! tree_has_app; then return 0; fi
    sleep 0.1
  done
  return 1
}

start_app() {
  local mode="$1"
  local log="$2"
  local args=()
  [ -n "$mode" ] && args+=("$mode")
  nested env XDG_DATA_HOME="$RT/data" XDG_STATE_HOME="$RT/state" \
    PATH="$RT/bin:$PATH" "$BINARY" "${args[@]}" >"$log" 2>&1 &
  APP_PID=$!
  PIDS+=("$APP_PID")
}

# A virtual keyboard is the only keyboard in the nested seat; keep one alive.
nested wtype -d 60000 -k Shift_L -k Shift_L >"$RT/keeper.log" 2>&1 &
PIDS+=($!)
sleep 0.3

# ---------------------------------------------------------------- launch

mkdir -p "$RT/data/applications" "$RT/state" "$RT/bin"
cat > "$RT/launch-target.sh" <<EOF
#!/bin/sh
touch "$RT/launched"
EOF
chmod +x "$RT/launch-target.sh"
cat > "$RT/data/applications/gofi-e2e.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Gofi E2E Launch
Exec=$RT/launch-target.sh
EOF

start_app "" "$EVIDENCE/launch-app.log"
wait_for_window || fail "launcher window did not open for the launch test"

nested wtype -d 80 "e2e"
sleep 0.4
nested grim "$EVIDENCE/launch-typed.png"
nested wtype -k Return

for _ in $(seq 1 50); do
  [ -e "$RT/launched" ] && break
  sleep 0.1
done
[ -e "$RT/launched" ] || fail "the fake desktop app was not launched"
wait_for_close || fail "the launcher stayed open after a successful launch"

STATE="$RT/state/gofi/state.json"
[ -f "$STATE" ] || fail "frecency state was not written"
jq -e '.modes.apps["apps:gofi-e2e.desktop"]' "$STATE" >/dev/null \
  || fail "the launched app was not recorded"
jq -e '.modes.run | length == 0' "$STATE" >/dev/null \
  || fail "run mode was recorded without a run launch"
grep -qi "panic" "$EVIDENCE/launch-app.log" && fail "launcher panicked (see launch-app.log)" || true
echo "PASS: typed query, launched the .desktop app, recorded frecency"

# ---------------------------------------------------------------- clipboard

cat > "$RT/bin/cliphist" <<EOF
#!/bin/sh
exec "$CLIPHIST" -db-path "$RT/cliphist.db" "\$@"
EOF
chmod +x "$RT/bin/cliphist"

SECRET="gofi-e2e-clipboard-secret-$RANDOM"
printf '%s' "$SECRET" | nested env PATH="$RT/bin:$PATH" cliphist store
nested env PATH="$RT/bin:$PATH" cliphist list | head -1 >/dev/null \
  || fail "the private cliphist database is empty"

start_app "clipboard" "$EVIDENCE/clipboard-app.log"
wait_for_window || fail "launcher window did not open for the clipboard test"

nested wtype -d 80 "clipboard"
sleep 0.4
nested grim "$EVIDENCE/clipboard-typed.png"
nested wtype -k Return
wait_for_close || fail "the launcher stayed open after a clipboard copy"

PASTED=""
for _ in $(seq 1 50); do
  PASTED=$(nested wl-paste 2>/dev/null || true)
  [ "$PASTED" = "$SECRET" ] && break
  sleep 0.1
done
[ "$PASTED" = "$SECRET" ] || fail "wl-paste did not return the selected entry"

if grep -qF "$SECRET" "$RT/state/gofi/state.json" 2>/dev/null; then
  fail "clipboard content leaked into the frecency state"
fi
if grep -qF "$SECRET" "$EVIDENCE/clipboard-app.log"; then
  fail "clipboard content leaked into the launcher log"
fi
if jq -e '.modes | has("clipboard")' "$RT/state/gofi/state.json" >/dev/null 2>&1; then
  fail "clipboard entries were persisted"
fi

echo "PASS: copied a cliphist entry and wl-paste returned it"
echo "PASS: no clipboard content or IDs in state or logs"
echo "evidence: $EVIDENCE"
