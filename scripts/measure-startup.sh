#!/usr/bin/env bash
# Measure process start -> Rune's first `on_ready` under a private nested Sway.
# Includes Rune/wgpu startup, the mode's provider scan and ranking; excludes
# compositor presentation.
#
# Usage:
#   scripts/measure-startup.sh --binary target/release/gofi [--mode apps] [--runs 10]

set -euo pipefail

BINARY=""
MODE=""
RUNS=10

while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-}"; shift 2 ;;
    --runs) RUNS="${2:-}"; shift 2 ;;
    -h|--help)
      echo "usage: scripts/measure-startup.sh --binary BIN [--mode apps|run|clipboard] [--runs N]"
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -n "$BINARY" ] || { echo "--binary is required" >&2; exit 2; }
[ -x "$BINARY" ] || { echo "not executable: $BINARY" >&2; exit 2; }
BINARY=$(realpath "$BINARY")

RT=$(mktemp -d /tmp/opencode/gofi-measure.XXXXXX)
chmod 700 "$RT"

SWAY_PID=""
cleanup() {
  [ -n "$SWAY_PID" ] && kill "$SWAY_PID" 2>/dev/null || true
  rm -rf "$RT"
}
trap cleanup EXIT

cat > "$RT/config" <<EOF
output HEADLESS-1 resolution 1920x1080
default_border none
for_window [app_id="gofi"] floating enable, border none, move position center, resize set 50 ppt 495 px
EOF

env -u SWAYSOCK -u I3SOCK \
  XDG_RUNTIME_DIR="$RT" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 sway -c "$RT/config" >"$RT/sway.log" 2>&1 &
SWAY_PID=$!

SOCK=""
for _ in $(seq 1 100); do
  SOCK=$(find "$RT" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit)
  [ -n "$SOCK" ] && break
  sleep 0.1
done
[ -n "$SOCK" ] || { echo "nested Sway did not start" >&2; exit 2; }

WL_DISPLAY=""
for _ in $(seq 1 100); do
  WL_SOCK=$(find "$RT" -maxdepth 1 -type s -name 'wayland-*' -print -quit)
  if [ -n "$WL_SOCK" ]; then WL_DISPLAY=$(basename "$WL_SOCK"); break; fi
  sleep 0.1
done
[ -n "$WL_DISPLAY" ] || { echo "nested Wayland display did not appear" >&2; exit 2; }

mkdir -p "$RT/state"
ARGS=()
[ -n "$MODE" ] && ARGS+=("$MODE")

RESULTS="$RT/results"
: > "$RESULTS"

for i in $(seq 1 "$RUNS"); do
  READY_DIR="$RT/ready-$i"
  mkdir -p "$READY_DIR"
  env -u SWAYSOCK -u I3SOCK \
    XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY="$WL_DISPLAY" \
    XDG_STATE_HOME="$RT/state" RUNE_READY_DIR="$READY_DIR" \
    "$BINARY" "${ARGS[@]}" >"$RT/app-$i.log" 2>&1 &
  APP_PID=$!

  READY=""
  for _ in $(seq 1 500); do
    READY=$(find "$READY_DIR" -maxdepth 1 -type f -print -quit)
    [ -n "$READY" ] && break
    sleep 0.01
  done

  if [ -n "$READY" ]; then
    jq -r '.ready_ms' "$READY" >> "$RESULTS"
  else
    echo "run $i: no ready file (see $RT/app-$i.log)" >&2
  fi

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  sleep 0.5
done

MODE_LABEL="${MODE:-apps}"
python3 - "$RESULTS" "$MODE_LABEL" <<'PY'
import statistics, sys
values = [float(line) for line in open(sys.argv[1]) if line.strip()]
if not values:
    print(f"{sys.argv[2]}: no measurements")
    raise SystemExit(1)
values.sort()
def pct(p):
    index = min(len(values) - 1, round((len(values) - 1) * p))
    return values[index]
print(f"{sys.argv[2]}: n={len(values)} min={values[0]:.1f} median={statistics.median(values):.1f} p95={pct(0.95):.1f} max={values[-1]:.1f} ms")
print("runs:", " ".join(f"{v:.0f}" for v in values))
PY
