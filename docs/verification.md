# Verification

The rule: **test behavior and logic that can break, through the real interface where
the interface is the point.** A test that restates the implementation costs
maintenance and catches nothing.

## Layers

| Layer | What it proves | Tools |
|---|---|---|
| Unit | parsing, ranking, frecency math, argv building, state round-trip | `cargo test`, temp dirs, fake clock |
| Integration | providers against real filesystem trees, launch against fake executables | `cargo test --test ...` |
| Headless UI | view structure, selection, query text, row count | Rune `--snap`, `--tree`, scenes |
| Real e2e | window appears, takes keys, launches, closes on focus loss | nested headless Sway + `wtype` + `swaymsg` + `grim` |
| Performance | cold/warm latency, query cost | measurement script, medians/p95 |

### What we test

- `.desktop` parsing: quoting, field codes, `Hidden`, `NoDisplay`, `OnlyShowIn`,
  `NotShowIn`, `TryExec`, `Terminal`, localized `Name`, precedence/dedupe.
- PATH scan: PATH order, symlinks, executable bit, duplicates, missing dirs,
  non-UTF-8 names (lossy is acceptable; do not panic).
- Ranking: fixed corpus + queries; exact/prefix/word-boundary beats fuzzy; smart
  case; Unicode; frecency promotes a tie but not a much better match; empty query
  sorts by frecency.
- Frecency: decay with a fake clock, update-on-use, saturation, eviction cap.
- State: missing file, corrupt JSON, unknown version, atomic write, lock, round-trip.
- Launch: argv exactly as parsed; quoted args preserved; field codes expanded;
  terminal wrapping; `Path=`; spawn failure is reported and does not panic.
- Clipboard: `cliphist list` parsing (id + preview, tabs, empty lines, 750 entries);
  fuzzy filter preserves recency order; `cliphist decode <id>` streams to `wl-copy`;
  missing/failing commands report without printing content; state and logs contain no
  previews or IDs.
- UI: the tree contains the expected rows, selected row, query; seven visible rows;
  no panic with an empty corpus or a missing icon.
- E2E: `gofi` window appears with `app_id="gofi"`, is focused, accepts typed text,
  `Enter` launches a fake app, `Escape` closes, focus loss closes.
- Clipboard e2e: seed a private cliphist database, open `gofi clipboard`, select an
  entry, verify `wl-paste` returns the exact content, and verify gofi state/logs do
  not contain it.

### What we do not test

- Exact fuzzy scores or private ranking fields. Assert order and invariants.
- Pixel-perfect screenshots while the UI is still moving. Save screenshots as
  evidence; assert the element tree instead. Add goldens only after the look is
  frozen.
- Image clipboard thumbnails in MVP; cliphist previews are text only.
- Obvious std behavior (e.g. that `Path::exists` works). Do not mock the filesystem;
  use a real temp tree.
- Implementation details that a refactor may legitimately change.

Every bug fix gets a regression test at the lowest layer that reproduces it. If the
bug is only visible in a real window, add it to the e2e suite.

## M1 result (2026-11-11)

- `just check` green: fmt, clippy `-D warnings`, 49 tests (3 unit + 16 provider +
  30 ranking/state).
- Real CLI on the owner's machine: 94 apps (all icons resolved), 500+ PATH
  executables, 750 cliphist entries.
- `--dump` end-to-end medians over 9 runs: apps **30 ms**, run **14 ms**, clipboard
  **15 ms** (process start, provider scan, ranking, JSON).
- Ranking a synthetic 5,000-item corpus: **0.8 ms** per pass including underline
  positions.
- Frecency verified through the CLI with a temp `XDG_STATE_HOME`: an app with five
  uses rose to the top of the empty-query list; a corrupt file and an unknown
  version produced a warning and an empty state, not a failure.

## Commands

`just` is installed. The current recipes:

```sh
just check        # fmt --check, clippy -D warnings, test
just test         # cargo test --workspace
just build        # cargo build --release -p gofi
just dump apps code
just dump run power
just dump clipboard ""
```

Planned for M2/M3: `just run`, `just snap`, `just e2e`, `just measure`.

The generic nested-Sway smoke test already exists:

```sh
scripts/e2e-sway.sh --binary target/release/gofi --query code \
  --evidence target/evidence/e2e
```

## Headless Rune

Rune apps run without a window and can render a PNG, dump the element tree and replay
scripted input. This is the right tool for UI structure, not for OS integration.

```sh
# Render the launcher in a named scene to a PNG.
cargo run -- --scene apps-code --snap target/evidence/m2/apps-code.png

# Dump the element tree at a virtual time.
cargo run -- --scene apps-code --tree target/evidence/m2/apps-code.json

# Scripted input: see Rune's examples/launcher/scripts/demo.json.
cargo run -- --script scripts/query-code.json --at 0.5 --snap out.png
```

Scenes should be deterministic: fixed corpus, fixed frecency, fixed scale, no
network, no clock. The app should expose scenes such as `apps-empty`,
`apps-code`, `run-git`, `no-results`, `long-names`.

## Nested headless Sway (real e2e)

The owner's live Sway session must never be touched. Use a fresh
`XDG_RUNTIME_DIR` and unset `SWAYSOCK`/`I3SOCK`; pass the nested socket to
`swaymsg` explicitly and assert it is under the temp dir.

`scripts/e2e-sway.sh` implements this recipe and was verified on 2026-11-10 with
the surface spike (`PASS`: app_id, focus, typing, Escape). The manual recipe below
is kept for debugging; prefer the script.

```sh
set -eu
RT=/tmp/opencode/gofi-e2e
rm -rf "$RT"; mkdir -p "$RT"; chmod 700 "$RT"

cat > "$RT/config" <<'EOF'
output HEADLESS-1 resolution 1920x1080
default_border none
for_window [app_id="gofi"] floating enable, border none, move position center, resize set 50 ppt 495 px
EOF

env -u SWAYSOCK -u I3SOCK \
  XDG_RUNTIME_DIR="$RT" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 sway -c "$RT/config" >"$RT/sway.log" 2>&1 &
SWAY_PID=$!

for _ in $(seq 1 50); do
  SOCK=$(ls "$RT"/sway-ipc.*.sock 2>/dev/null | head -1 || true)
  [ -n "${SOCK:-}" ] && break
  sleep 0.1
done
[ -n "${SOCK:-}" ] || { echo "nested sway did not start"; exit 1; }
case "$SOCK" in "$RT"/*) ;; *) echo "refusing foreign socket: $SOCK"; exit 1 ;; esac

env -u SWAYSOCK -u I3SOCK XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY=wayland-1 \
  target/release/gofi >"$RT/gofi.log" 2>&1 &
APP_PID=$!

sleep 1
env -u SWAYSOCK -u I3SOCK swaymsg -s "$SOCK" -t get_tree \
  | jq -r '.. | objects | select(.app_id? == "gofi") | [.name, .app_id, .focused] | @tsv'

env -u SWAYSOCK -u I3SOCK XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY=wayland-1 \
  grim "$RT/shot.png"

# Type a query and press Enter. Use a delay so the first key isn't lost while the
# text input takes focus.
env -u SWAYSOCK -u I3SOCK XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY=wayland-1 \
  wtype -d 80 "code"
sleep 0.3
env -u SWAYSOCK -u I3SOCK XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY=wayland-1 \
  wtype -k Return

kill "$APP_PID" "$SWAY_PID" 2>/dev/null || true
```

### Virtual keyboard lifetime (important)

`wtype` creates a `zwp_virtual_keyboard_v1`. In a nested Sway started with
`WLR_LIBINPUT_NO_DEVICES=1` there is **no physical keyboard**, so when `wtype`
exits, Sway removes the virtual keyboard and the seat loses keyboard capability.
winit reports that as `WindowEvent::Focused(false)`, which a launcher that closes
on focus loss will interpret as a real focus loss. The window is still focused in
Sway, but the keyboard is gone.

For a focus-loss test, keep a virtual keyboard alive while another window steals
focus:

```sh
# Sends 'a', sleeps 5 s, sends 'b'; the process keeps the keyboard alive.
wtype -d 5000 -k a -k b &
sleep 0.4
# Launch another window; the launcher should close on the real focus loss.
hello &
```

For typing tests, use `-d` between keys and take the screenshot before `wtype`
exits. In a real session with a physical keyboard, this artifact does not occur;
a keyboard unplug is a legitimate reason to close the launcher.

This was found during the M0 surface spike (2026-11-10). The launcher surface
spike verified `app_id="gofi"`, the Sway float/centre rule, text input, Escape and
focus-loss close with a live virtual keyboard.

The script asserts the target geometry: 960×495 at 480,293 on a 1920×1080 output.
That is 50% width × 495 logical height, centred.

Notes:

- `swaymsg -s <socket>` is required; `SWAYSOCK` may still point at the live session.
- The nested output is `HEADLESS-1` in this setup; check `swaymsg -s "$SOCK" -t
  get_outputs` if it differs.
- `wtype` uses the virtual-keyboard protocol; Sway supports it. If it fails, the
  fallback is `swaymsg` `exec` + a test-only key injection path, not the live seat.
- The app must call `rune::set_app_id("gofi")` for the window rule and tree queries.
- `grim` needs `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` of the nested compositor.
- Keep each test's runtime dir unique; remove it afterwards.
- Evidence: `target/evidence/<task>/` (logs, tree JSON, screenshots, timings).

## Real-config measurement (rofi)

To check a size/position claim against the owner's actual config, copy
`~/.config/sway` to a temp dir, remove `include ./startup.conf` (so no apps start),
append a headless output, and run nested Sway with that config:

```sh
cp -a ~/.config/sway "$RT/sway"
sed -i '/include .\/startup.conf/d' "$RT/sway/config"
printf '\noutput HEADLESS-1 resolution 1920x1080 scale 1\n' >> "$RT/sway/config"
```

Then launch the real `rofi -show drun` with `DISPLAY` unset and the nested
`WAYLAND_DISPLAY`, capture with `grim`, and measure the panel bbox (background
`#1e2030`). Rofi is layer-shell, so it does not appear in `swaymsg -t get_tree`.

Verified results:

| Output | Rofi panel | Gofi toplevel with the rule |
|---|---|---|
| 1920×1080 @ 1 | 960×495 at 480,293 | 960×495 at 480,293 |
| 2560×1600 @ 1.5 | 1279×742 physical at 641,429 | 853×495 logical at 427,286 (1280×742 physical) |

Use 1920×1080 for the normal e2e; use 2560×1600 @ 1.5 when checking scale.

## Clipboard e2e (planned)

Clipboard must not touch the owner's real history. Isolate it in the nested
session:

1. Put a wrapper named `cliphist` first in `PATH` that calls the real binary with
   `-db-path "$RT/cliphist.db"` and passes `"$@"`. This keeps gofi's command line
   unchanged while the database lives in the temp dir.
2. Seed with `printf 'gofi-test-value' | cliphist store`.
3. Start `gofi clipboard` under the nested Sway, wait for the row, select it.
4. `wl-paste` in the nested session must return `gofi-test-value`.
5. Assert `target/evidence/<task>/app.log` and `state.json` do not contain the value.

The nested compositor isolates `wl-copy`/`wl-paste` from the live session; do not
run clipboard tests against the live `WAYLAND_DISPLAY`.

Verified 2026-11-10 with a private DB: `cliphist store` → `list` (id + preview) →
`decode` → `wl-copy` → `wl-paste` returned the test value under nested Sway.

## Performance measurement

Targets are in `goals.md`; measure, do not assert in unit tests.

### Cold start

Rune writes a readiness file when `RUNE_READY_DIR` is set:

```
$RUNE_READY_DIR/<name>.<pid>  →  {"pid":...,"ready_ms":...,"rune":"..."}
```

Run the real binary 10 times from a quiet nested Sway, parse `ready_ms`, report
median and p95. State exactly what is included (process start → first `on_ready`),
and what is not (compositor presentation, input latency). Do not claim
"first pixel" unless measured.

### Warm toggle (if the daemon is adopted)

Instrument the daemon to append `show_requested` / `window_ready` timestamps to a
file (test build only), or measure `gofi toggle` round-trip until `swaymsg` reports
the window focused. Report median/p95 over 20 toggles.

### Query latency

Add a `gofi bench --mode run --items 5000 --queries 1000` subcommand or a
`#[bench]`-style binary. Score a fixed corpus, report p50/p95, and compare before/after
a ranking change. Do not gate CI on wall-clock numbers; gate on gross regressions
(>2×) only when the machine is quiet.

## Evidence and reporting

Every worker/verifier saves:

```
target/evidence/<task>/
  commands.log
  check.log          # fmt/clippy/test/build
  tree.json          # headless or nested Sway
  before.png
  after.png
  timings.json
  notes.md
```

The final report references these paths. Raw logs stay in the files; the chat report
contains the result, the command, and the artifact path.
