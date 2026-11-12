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

## M2/M3 result (2026-11-11)

- `just check` green: 66 tests (17 binary/UI + 3 unit + 16 provider + 30
  ranking/state).
- Headless Rune tests assert the window geometry (row 902×48 at 20,89; tab
  100×59 at 600,10; scrollbar 8×396 at 932,89), seven rows, three tabs, typing,
  Tab/Shift+Tab, arrow wrap and scroll window, wheel direction, empty corpus,
  clipboard order, underline byte spans with Unicode, and a failed launch notice.
- Nested Sway: `scripts/e2e-sway.sh` PASS in apps and run modes (focus,
  960×495, typing, Escape, and a second invocation closing the open launcher);
  `scripts/e2e-launch.sh` PASS (fake `.desktop` app launched and recorded;
  private cliphist entry copied, `wl-paste` returns it; no content in state or
  logs). Screenshots: `target/evidence/m2-toggle/`, `target/evidence/m2-run/`,
  `target/evidence/m2-actions2/`.
- Startup (`scripts/measure-startup.sh`, process start → first `on_ready`, nested
  pixman Sway): apps median 300 ms, p95 350 ms, max 380 ms over 20 runs; run
  230–250 ms; clipboard 190–280 ms. Compositor presentation is not included.
- The UI uses the embedded JetBrains Mono Regular/Medium. With the system family
  lookup the apps p95 was 430 ms; embedding removed the fontconfig cost.

## Polish result (2026-11-11)

- `just check` green: 75 tests (23 binary/UI + 3 unit + 19 provider + 30
  ranking/state).
- Rofi 2.0 was run in a private nested Sway (isolated `XDG_RUNTIME_DIR`,
  `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, no live socket) to settle
  the target behavior: `scroll-method: 1` keeps the selected row in the middle of
  the seven-row window, the scrollbar tracks the selection, the handle length is
  `track * 7 / (total + 7)`, and a short list keeps a gap below.
- New headless tests cover: `scroll_offset` against rofi's algorithm; scrollbar
  height, position, right edge and bottom stop; the short-list handle and gap;
  Ctrl+N/P; Home/End/PageUp/PageDown; Ctrl+W/U/K with the caret mid-line;
  Shift+Right/Left mode cycling; and the search glyph.
- Visual proof under nested Sway: `target/evidence/polish-e2e/typed.png` and the
  private probe in `/tmp/opencode/gofi-probe/` (not committed): complete
  magnifier, centered selection after seven and twenty-seven Downs, a scrollbar
  that moves down without drifting left, Ctrl+N, Ctrl+W and Ctrl+U.
- Icons: the resolver follows GTK's theme chain and legacy categories. `--dump`
  now resolves 93 of 94 apps; `hardware locality lstopo` (`Icon=hwloc`) has no
  installed icon and rofi shows none either.
- Startup with the cached theme lookups: apps median **300 ms**, p95 **330 ms**
  (20 runs, nested pixman); first two runs 140–180 ms.
- The two e2e scripts still pass (`target/evidence/polish-e2e/`,
  `target/evidence/polish-actions/`).

## Icon index result (2026-11-11)

- Resolving 94 app icons walked Adwaita and AdwaitaLegacy in full for each name
  before reaching hicolor, where 72 of the 94 live. `Icons` now indexes the
  plain layouts and every theme in the chain once per app scan; precedence is
  unchanged and all 94 resolved paths are byte-identical.
- `--dump --mode apps`: **117 ms → 23 ms** median (12 runs, warm cache), 94
  apps, 93 icons (`hwloc` absent as before).
- Startup, spawn → first buffer commit, alternating 16 runs in a private nested
  Sway: gofi median **124 ms**, rofi 2.0 median **59 ms**; paired gap **+65 ms**,
  down from +172 ms before the index. Absolute values move with session cache
  state; the paired gap is the stable figure.
- `just check` green (75 tests); both e2e scripts pass
  (`target/evidence/icons-index/`, `target/evidence/icons-index-actions/`).

## Match result (2026-11-11)

- `--query tgram`: Telegram first (107), GIMP second (83), every row with five
  highlighted characters. Before, LibreOffice Draw's keyword match won with an
  empty highlight. Rofi 2.0 with the same corpus ranks GIMP first and Telegram
  second, so gofi now prefers the tighter match.
- `--query fire`: Firefox, LibreOffice Impress, LibreOffice Writer — the same
  order and the same highlighted runs as rofi 2.0 in the nested probe.
- `--query code`: rofi puts OpenCode first (a camel-case bonus); gofi ties T3
  Code and Visual Studio Code above OpenCode. The matchers still differ on
  camel-case boundaries; no invisible matches remain.
- Matched runs are weight 700 plus underline, rofi's default
  `ROFI_HL_BOLD | ROFI_HL_UNDERLINE`; screenshot
  `target/evidence/match-bold/typed.png`. Rofi's underline is 2 px because Pango
  derives it from the font; Rune's span underline is fixed at 1 px
  (`scale.max(1.0)`), so the bold run is what closes the visual gap.
- `just check` green (76 tests).

## Install and real-config result (2026-11-12)

- Installed at `~/.local/bin/gofi` with `just install`. The installed copy passes
  both e2e scripts (`target/evidence/install-apps/`,
  `target/evidence/install-actions/`).
- Ran the installed binary under nested Sway with the owner's actual config
  (startup and local `exec` includes commented out). `move position center` centred
  in the workspace, not the output: `gaps inner 8` made the window **952×495 at
  484,310** instead of rofi's 960×495 at 480,293.
- The rule adds `resize grow width 8 px` to cancel this config's gap inset.
  `move absolute position center` was tried and rejected: Sway centres it across the
  whole output layout, so with two outputs the panel landed between them.
- With `move position center` and two nested outputs, launching through
  `swaymsg exec` (the keybind path) put the panel on the focused output:
  **960×495 at 2186,292** on 1920×1080 and **853×495 at 426,285** on
  2560×1600 @ 1.5. A bar makes it centre in the workspace below the bar, so it can
  sit lower than rofi; exact output centring needs a layer-shell surface.
- With the owner's full config on two nested outputs, launched through
  `swaymsg exec`: 853x495 at 426,285 on the focused 2560x1600 @ 1.5 output and
  960x495 at 2186,292 on the focused 1920x1080 output. The panel follows the
  focused workspace. `target/evidence/real-config-dual/`.
- Long clipboard previews (cliphist caps them at 100 chars) used to widen the list
  and push the scrollbar to the window edge. Rows now truncate with `…` and the
  list can shrink: the scrollbar sits at x 932 in both modes (1412-1419 px in the
  1920 capture, `target/evidence/scrollbar/clipboard-fixed.png`). Regression test
  `long_clipboard_titles_do_not_push_the_scrollbar`.
- Startup sizing: gofi asks Sway for the focused output's width and requests half
  of it. On 2560×1600 @ 1.5 the first mapped frame was already **853×495** (before,
  960 then 853 one frame later); on 1920×1080 it was **960×495**. Logs:
  `target/evidence/no-resize/scaled.log`, `target/evidence/no-resize/hd.log`.
- `just check` green (80 tests).

## Startup breakdown (2026-11-11)

Spawn → first buffer commit, 12 runs, nested headless Sway. The client renders
with the real GPU (`RADV PHOENIX`); only compositing is pixman.

| Phase | Median |
|---|---|
| spawn → Wayland connect | 3.6 ms |
| connect → first window surface | 63.9 ms |
| window → first Vulkan buffer | 50.6 ms |
| buffer → commit | 7.7 ms |
| **total** | **126.2 ms** |

The `connect → window` slice includes the app provider and icon index (about
20 ms; `--dump` is 17 ms without icons and 23 ms with). Rune's GPU prewarm is a
no-op on Linux; the rest is winit/font/window setup, wgpu device and swapchain,
pipelines and the first draw. Rofi 2.0 in the same session: 57.9 ms median.

## Startup optimization (2026-11-12)

`ready_ms` is process start → Rune's first `on_ready`, measured under nested
headless Sway with `scripts/measure-startup.sh --runs 20`:

| | median | p95 |
|---|---|---|
| before | 170 ms | 180 ms |
| after | **140 ms** | **150 ms** |

What changed:

- `init` starts the mode provider on a thread and returns; the first `poll` takes
  the result. The scan (~20 ms warm, ~30 ms cold) now overlaps Rune's window and
  GPU setup instead of blocking `init`.
- `rune::prewarm_gpu()` was called before `App::run`, but in the pinned Rune its
  Linux implementation is empty (`render/warm.rs::start`), so it changed nothing.

The remaining phase split (`WAYLAND_DEBUG=1`, 10 runs, logging overhead included)
is spawn → connect 3.4 ms, connect → xdg_surface 64.5 ms, xdg → first buffer
68.3 ms, buffer → commit 12.5 ms. The app-side work is no longer on the critical
path. Rofi 2.0's full frame was measured at 58 ms; one-shot gofi is still about
2x that. The next lever is the resident daemon (ADR 0007), not more provider
work.

`GOFI_TRACE=1 gofi` prints wall-clock markers for `main`, `init`, the preload and
`on_ready`; `WAYLAND_DEBUG=1` adds the protocol phases.

## Rune startup patch (2026-11-13)

A patch to Rune removes work that every launch paid for whether gofi used it or
not. `ready_ms` under nested headless Sway with `scripts/measure-startup.sh
--runs 20`:

| build | median | p95 |
|---|---:|---:|
| pinned Rune | 140 ms | 170 ms |
| patched Rune | **80 ms** | **90 ms** |
| rofi 2.0, same session | 58 ms | — |

The patch (`research/rune-startup.patch`, 9 files, 138 insertions) changes:

- **CJK and symbol fallbacks load on first use.** Rune eagerly loaded up to 16
  installed CJK files and the symbol/emoji families during `TextSystem::new`.
  Here that was ~78 ms per launch even for a Latin-only query. Lazy loading was
  verified with `日本語テスト` and `🚀 Rocket` `.desktop` entries: both render,
  including colour emoji.
- **`fc-list` runs only when the font directories have none of the requested
  files.** The directory walk is ~3 ms; `fc-list` was ~40 ms here. NixOS store
  paths still work because the walk finds nothing and falls back to fontconfig.
- **Image decoder threads start with the first image request**, not at
  `ImageCache::new` (~14 ms). Icons still load; they are requested during the
  first frame, after the thread spawn is cheap.
- **The reduced-motion preference is read after the first frame**, and cached
  per process. On Linux it loads GIO and its schemas (~10 ms).
- **Adapter selection asks for an adapter before enumerating.** On this machine
  `request_adapter` is ~1.6 ms where `enumerate_adapters` was 10–35 ms; the
  enumeration remains the fallback when the requested adapter is not the
  compositor's GPU.

Phase timings from the instrumented build (no `WAYLAND_DEBUG`, 8 runs, medians).
The rows overlap (for example, the image cache and preference read are inside
`Host::new`), so they are individual costs, not a sum:

| phase | before | after |
|---|---:|---:|
| end to end (`ready_ms`) | 140 ms | 80 ms |
| `TextSystem::new` + fallbacks | 81 ms | 3 ms |
| `ImageCache::new` | ~14 ms | <1 ms |
| reduced-motion read | 10 ms | after first frame |
| window creation | 7 ms | 7 ms |
| wgpu instance + adapter + device + pipelines | ~50 ms | ~50 ms |
| surface config + first frame | ~16 ms | ~16 ms |

The GPU work is now the long pole. A Linux implementation of `prewarm_gpu()` was
built and measured: it warms a device and renderer on a thread, but with the text
work gone the window waits for the same ~50 ms, and total `ready_ms` stayed at
80/90 ms. It is not in the patch.

The first draw still waits for the compositor's first configure (~15 ms in the
nested session). That, plus fresh-process GPU init, is the remaining gap to
rofi. The resident daemon (ADR 0007) removes both by keeping the device,
renderer and providers warm.

Verification: `just check` green (80 tests), both nested-Sway e2e scripts pass,
CJK and emoji screenshots captured in `target/evidence/rune-patch-cjk/` and
`target/evidence/emoji/`. The patch applies cleanly to the pinned Rune checkout
(`04b38ae0a73bda03f010e97d2fff540ab91f67b0`) with `git apply`. It is **not upstreamed** and the repo still builds against
`references/rune`; installing the fast build requires applying the patch to the
Rune checkout.

## M1 result (2026-11-11)

- `just check` green: fmt, clippy `-D warnings`, 49 tests (3 unit + 16 provider +
  30 ranking/state).
- Real CLI on the owner's machine: 94 apps (93 icons; `hwloc` ships none),
  500+ PATH executables, 750 cliphist entries.
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
just check        # rune pin, fmt --check, clippy -D warnings, test
just test         # cargo test --workspace
just build        # cargo build --release -p gofi
just run          # open the launcher in apps mode
just run-mode run
just dump apps code
just dump run power
just dump clipboard ""
just e2e          # nested Sway: geometry, focus, typing, Escape
just e2e-launch   # nested Sway: launch a .desktop app, copy cliphist
just measure apps # startup, 20 runs
```

The generic nested-Sway smoke test:

```sh
scripts/e2e-sway.sh --binary target/release/gofi --query fire --mode apps \
  --evidence target/evidence/e2e
```

It accepts y=292 or 293 on a 1920×1080 output: 495 is odd, so the centred y is
292.5; Sway floors a toplevel to 292 while layer-shell rounds rofi up to 293.
Both are the same centred position.

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
for_window [app_id="gofi"] floating enable, border none, resize set 50 ppt 495 px, move position center
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
