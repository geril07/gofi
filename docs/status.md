# Status

**Last updated:** 2026-11-14
**Phase:** M3 complete → M4 — Sway integration and daily use

## Done

- Reconnaissance of the current setup:
  - rofi 2.0.0, Catppuccin Macchiato, JetBrains Mono 16, 32 px icons, 7 rows.
  - Sway keybinds: `Mod+Return` drun, `Mod+Shift+Return` run, `Mod+Shift+w` window,
    `Mod+Ctrl+c` clipboard.
  - rofi config copied to `references/rofi/`; Sway config to `references/sway/`.
- Cloned Rune at `04b38ae0a73bda03f010e97d2fff540ab91f67b0` to `references/rune`.
- Cloned rofi 2.0.0 source to `references/rofi-src`.
- Installed Rust 1.99.1; Rune `cargo check -p rune` passes.
- Built `examples/hello`; rendered a real Rune window under nested headless Sway
  (wgpu + pixman, 1280×800) and captured a screenshot. Sway floated/centred it.
- Measured `hello` readiness under nested Sway: debug **280–310 ms** (5 runs);
  release **90 ms median** (9 warm runs, 240 ms first run). This is process start →
  first `on_ready`, not compositor presentation. One-shot process is viable; the
  daemon is not needed for MVP (ADR 0007).
- Launcher surface spike passed (Rune 700×480 borderless transparent window; the
  real target is 960×495):
  - `rune::set_app_id("gofi")` → Sway sees `app_id="gofi"`.
  - `for_window [app_id="gofi"] floating enable, border none, move position center`
    + `resize set 700 480` → exactly 290,160 700×480 on a 1280×800 output.
  - Text input receives typed keys; `Escape` closes; focus loss closes when a real
    focus change occurs.
  - Harness caveat: `wtype` owns the only keyboard in nested headless Sway; when it
    exits, the seat loses keyboard capability and Rune reports focus loss. Keep a
    virtual keyboard alive during focus-loss tests. Real sessions with a physical
    keyboard are unaffected. Documented in `verification.md`.
- E2E harness: `scripts/e2e-sway.sh` starts a nested headless Sway, asserts the
  socket is private, checks `app_id`/focus, captures screenshots, types a query,
  and checks Escape closes the window. Verified with the surface spike:
  `PASS` (evidence in `target/evidence/e2e-spike/`).
- Owner provided screenshots (`references/screenshots/`, gitignored). Measured the
  real look: **960×495** panel, 902×48 selected row, 57 px pitch, 100×59 mode tabs
  with 21 px gap, 8 px scrollbar, matched characters underlined. The tabs are the
  mode switcher; the folder/filebrowser tab is dropped.
- **Clipboard is in MVP.** It uses the existing `cliphist` database and
  `cliphist decode | wl-copy`; no own history, no clipboard content in state/logs.
  ADR 0008. Verified the pipeline with a private DB in nested Sway:
  `store` → `list` → `decode` → `wl-copy` → `wl-paste` returned the test value.
- Real-config geometry check: ran the owner's actual Sway config (startup include
  removed) in nested Sway with the real rofi. Rofi is layer-shell, 50% width ×
  495 logical height, centred: 960×495 at 480,293 on 1920×1080; 853×495 logical
  (1279×742 physical) on 2560×1600 @ 1.5. Verified at both outputs.
- Installed it (`~/.local/bin/gofi`, `just install`) and re-checked geometry against
  the owner's real config: plain `move position center` centred in the workspace,
  which sits below the bar and inside `gaps inner 8` — 952×495 at 484,310 instead
  of rofi's 960×495 at 480,293. The rule adds `resize grow width 8 px` to cancel the
  horizontal gap inset (owner-config-specific; the README documents it).
- Tried `move absolute position center` for rofi's vertical centring. It is wrong on
  multiple outputs: Sway centres across the whole output layout, not the focused
  one (measured x 2293 with three test outputs). Back to `move position center`,
  which follows the focused workspace. Verified through `swaymsg exec` (the keybind
  path) with two outputs: **960×495 at 2186,292** on the focused 1920×1080 output and
  **853×495 at 426,285** on the focused 1706×1066 (2560×1600 @ 1.5) output.
- A bar shifts the workspace-centred panel down by about half the bar height,
  because rofi's layer-shell surface centres on the output. Exact parity needs the
  layer-shell launcher surface (see `rune-assessment.md`), not a Sway rule.
- Wrote the initial docs: orchestration, goals, architecture, verification, ADRs
  0001–0008, this status, and the Rune assessment.
- **M1 core complete** (`b27561f` providers, ranking/state integrated next commit):
  - Providers: `.desktop` discovery (XDG precedence, Hidden/NoDisplay,
    OnlyShowIn/NotShowIn, TryExec, locale fallback, field codes, `Terminal=true`,
    `Path=`), `$PATH` scan, `cliphist list`; icons resolve for 93 of 94 apps on the
    owner's machine (`hwloc` ships none).
  - Ranking: nucleo smart case/normalization, composite
    `match + 0.15 * top * normalize(frecency)`, title tie-break case-insensitive,
    clipboard order preserved; underline positions are character indices.
  - State: JSON v1, 14-day half-life, atomic write with a sidecar `flock`,
    2,000-entry cap, no clipboard persistence.
  - 49 behavior tests (3 unit + 16 provider + 30 ranking/state).
  - Real CLI: 94 apps, 500+ executables, 750 cliphist entries.
  - End-to-end `--dump` (process start + scan + rank): apps **30 ms**,
    run **14 ms**, clipboard **15 ms** median on the owner's machine. Ranking a
    synthetic 5,000-item corpus: **0.8 ms** per pass including positions.
- **M2/M3 complete**: the Rune window and the actions.
  - View: mantle panel, 20 px insets, search field, three 100×59 tabs, seven
    48 px rows on a 57 px pitch, 32 px icons, 8 px scrollbar, match underlines.
    The measured geometry matches rofi (panel 960×495, row 902×48, tabs 100×59).
  - JetBrains Mono Regular/Medium are embedded (SIL OFL) so startup does not pay
    a fontconfig lookup. Body text is 16 pt at 96 DPI = 21.33 px, matching the
    owner's rofi.
  - Keys: Up/Down wrap, Tab/Shift+Tab cycle modes, Enter activates, Escape and
    Ctrl+G/Ctrl+C/Ctrl+[ close; wheel moves one row; row click activates; focus
    loss closes. Mode switching clears the query and lazily loads that provider.
  - Actions: detached launch, `cliphist decode | wl-copy`, frecency update and
    save; failures stay in a notice row. Single instance by `flock`, second
    keybind sends SIGTERM (ADR 0009).
  - 17 binary/UI tests (headless Rune `Runtime` plus launch/instance with real
    temp trees and fake executables), 49 core tests: **66 total**.
  - E2E under nested headless Sway: `scripts/e2e-sway.sh` PASS (geometry, focus,
    typing, Escape); `scripts/e2e-launch.sh` PASS (launch a fake `.desktop` app
    and record frecency; copy a private cliphist entry and `wl-paste` returns it;
    no clipboard content in state or logs).
  - Cold start, process start → first `on_ready`, nested headless Sway, 20 runs:
    apps **median 300 ms, p95 350 ms, max 380 ms**; run 230–250 ms; clipboard
    190–280 ms. This excludes compositor presentation.
  - The app sits at y 292 on 1920×1080, not rofi's 293: 495 is odd and Sway
    floors a toplevel's centred position while layer-shell rounds up. Same
    centred position, 1 px difference.
- **First real-use polish** (2026-11-11), checked against the owner's rofi 2.0 in a
  private nested Sway:
  - Scrolling now matches `scroll-method: 1`: the selection stays centered once it
    passes the fourth row, the list follows, and the end clamps to a full page.
    Short lists keep rofi's gap below.
  - The scrollbar was a row, so its handle was positioned on the horizontal axis
    and never moved down; it is now an absolutely positioned handle that tracks the
    selected row with rofi's length formula. The left drift is gone.
  - Keys: `Ctrl+N`/`Ctrl+P` rows, `Home`/`End`, `PageUp`/`PageDown`,
    `Shift+Right`/`Shift+Left` modes, `Ctrl+A`/`E`/`B`/`F` caret, and
    `Ctrl+W` (clear), `Ctrl+U` (delete to start), `Ctrl+K` (delete to end).
  - The search-tab magnifier is drawn as a ring plus a round-capped handle instead
    of a ring with a detached rotated bar.
  - Icon lookup now follows GTK's theme chain (the configured theme, its
    `Inherits`, `AdwaitaLegacy`, `hicolor`) and searches every declared size and
    category. All apps get an icon except `hwloc` (`lstopo`), which ships none and
    also has none in rofi.
  - Icon lookup indexes the theme chain once per app scan instead of walking it
    per name: `--dump --mode apps` dropped from 117 to 23 ms and the first frame
    by about 100 ms, with identical paths.
  - Apps now match `Name` only, as rofi's `drun-match-fields: name` does. A query
    can no longer return a keyword-only row with nothing to underline (`tgram`
    now puts Telegram first).
  - Matched characters are bold and underlined, matching rofi's default highlight
    (`ROFI_HL_BOLD | ROFI_HL_UNDERLINE`).
  - Long clipboard previews no longer push the scrollbar: rows truncate with `…`
    and the list shrinks, so the scrollbar sits at the same x as apps/run.
  - Startup asks Sway for the focused output's width and requests half of it, so
    the 50% rule no longer resizes the first frame (853 on the 1.5 output).
    Sway IPC parsing tests; **80 tests** total.
  - `just install` copies the release binary to `~/.local/bin/gofi`; the installed
    copy passes both nested-Sway e2e scripts. Sway config remains the owner's step.
  - Startup optimized: the mode provider loads on a thread from `init` and the
    first frame takes the result. `ready_ms` is **140 ms median, 150 ms p95** over
    20 runs (was 170/180). **80 tests** total. `rune::prewarm_gpu()` is a no-op on
    Linux in the pinned Rune; it was not the source of that gain.
  - Rune startup patch (not upstreamed): lazy CJK/symbol fallbacks, `fc-list` only
    when the font dirs have no match, lazy image workers, a deferred
    reduced-motion read, and a cheaper adapter pick. With the patch, `ready_ms` is
    **80 ms median, 90 ms p95** over 20 runs. Measured to the first commit that
    carries pixels, rofi is ~89 ms, so the patched build is level with it; the
    earlier 58 ms rofi figure was its empty configure commit. Patch:
    [`research/rune-startup.patch`](research/rune-startup.patch). The remaining
    fixed cost is Rune/wgpu's fresh-process GPU init and the compositor's first
    configure, which the resident daemon would amortize.
  - Underline fixed with a second Rune patch: `Span::underline_width` lets a
    span name its thickness in points, and gofi's matched spans use 2 points.
    At 1x the line is 2 px like rofi's; at 1.5x it is 4 px. Evidence:
    `target/evidence/underline-installed/typed.png` (1x) and
    `target/evidence/underline-scale-1.5/typed.png`. Patch:
    [`research/rune-underline.patch`](research/rune-underline.patch).
  - The patches are now the build dependency: `scripts/setup-rune.sh` creates the
    gitignored `vendor/rune` from `references/rune` and applies every
    `docs/research/rune-*.patch`; `just build`/`just check` run it.
    `~/.local/bin/gofi` is the 80 ms build, `references/rune` stays pristine, and
    `just check-rune` verifies the pin and every patch. Upstreaming the Rune
    patches is deferred.

## UI refresh (2026-11-14)

- Replaced the rofi-parity skin with the quiet **800×496** panel: a tinted
  selection pill at the selected slot plus a 2 px accent rail, text tabs with a
  40×2 underline, 12 px panel radius, hairline border. No metadata column, no
  footer, no preview pane.
- The fixed size removed the `swaymsg` output query. The Sway rule is now
  `resize set 800 px 496 px`; e2e asserts 800×496 at 560,292.
- Added `ui/motion.rs`: selection glide 0.24 s, tab shared-element flight 0.22 s,
  and a mode swap that slides the incoming list 10 px (0.18 s) while the outgoing
  one fades (0.14 s). Typing stays instant and there is no open animation.
  `GOFI_MOTION=off` returns `Spring::NONE` / `Transition::none()` and
  marks the root `instant_changes`; resting screenshots with motion on and off are
  byte-identical (`target/evidence/ui-final*`).
- A full-panel `clip_rounded` cost ~10–15 ms on the first frame; the rounded fill
  and border remain, and children are inset so nothing needs clipping.
- `just check` green (**81 tests**). E2E on/off, launch/copy, and the CJK query
  pass. Startup is **80 ms median / 100 ms p95** (20 runs, installed build); the
  previous installed build measured 90 ms median under the same harness in the
  same session. Installed at `~/.local/bin/gofi`.
- Decision: [ADR 0010](decisions/0010-quiet-motion.md).

## Tab and motion refinement (2026-11-14)

Owner review of the first skin: tighten the tabs, stop the underline from lifting
the selected label, drop the rail, and make the selection snappier.

- Tabs now shrink-wrap their labels with an 18 px gap (was three 96 px cells with
  8 px), and the underline is as wide as the label (38/28/70 px, was 40).
- The underline is absolutely positioned at a fixed y, so selecting a tab no
  longer moves its text up by 4 px. Unit test asserts the label's rect is
  unchanged across a mode switch.
- Removed the 2 px rail inside the selection pill; the 16% blue tint alone marks
  the row.
- Retuned motion: selection **0.17 s / 0.25 bounce** (was 0.24 / 0), tab flight
  **0.16 s / 0.2 bounce** (was 0.22 / 0), mode swap **0.14 / 0.11 s** (was
  0.18 / 0.14). The pill now reaches the next row in ~0.1 s and settles by
  ~0.2 s, with a ~1.6 px overshoot; the old critically damped spring took
  ~0.38 s to settle.
- The search row can shrink, so a long query scrolls inside the field instead of
  widening it and pushing the tabs off the panel. Unit test types 120 chars and
  asserts the tab rect and field width are unchanged.
- `just check` green (**86 tests**). E2E on/off still passes with byte-identical
  resting screenshots (`56da2cd1…`), and launch/copy still passes. A 60-char
  query leaves the tabs in place (`target/evidence/long-query-fixed/`). Installed
  at `~/.local/bin/gofi` (sha256 `e6440375…`). Evidence `target/evidence/ui-tabs*`.

## Demo assets (2026-11-14)

- The README now leads with a 9.4 s inline MP4 player (1920×1200, 60 fps), with
  no still image or download links. It covers a query, paging, tab switches and
  clipboard scrolling. GitHub only plays uploaded user assets inline, so the
  player URL is an upload of `docs/assets/demo.mp4`; the file stays in the repo
  as the source.
- Recorded headlessly through Rune's own tooling (`--script`, `--video`,
  `--desktop`, `--scale 2`), not a compositor or screen capture. `gofi --record`
  hands the remaining flags to Rune.
- `scripts/record-demo.sh` / `just demo` rebuilds the fixtures, private cliphist
  DB and assets; nothing from the live clipboard, app list or gofi state is read.

## Ranking tiebreak (2026-11-14)

- Equal match scores now break by the first matched character's position in the
  title before the title itself, so a match at offset 8 no longer beats one at 0
  on its name alone. Apps and Run only; Clipboard keeps cliphist's order.
- A clearly better match still wins: for `ed`, `Text Editor` (62, start 5) stays
  above `Redshift` (36, start 1). The tiebreak only fires on exact ties.
- Real app list, `-q te`: Telegram and Text Editor (62, start 0) now precede
  "Qt V4L2 test Utility" (62, start 8).
- `word_boundary_matches_beat_mid_word_matches` changed: its two mid-word matches
  tie at 76, so "Xterm Settings Manager" (start 1) now precedes "Determinate"
  (start 2).
- Clipboard match-quality sorting is a gap, not a decision: issue
  [#6](https://github.com/geril07/gofi/issues/6). ADR 0008 is unchanged until it
  is resolved.
- ADR 0004 amended; three tests pin the tiebreak. `just check` green (**89 tests**).

## In progress

Nothing. **M4 — Sway integration and daily use — is next.**

## Next (ordered)

1. **Sway integration**: `~/.local/bin/gofi` is the new build; replace the window
   rule with `resize set 800 px 496 px` and point the keybinds at gofi. No config
   is changed by this project.
2. **Daily-use checklist** for one week, keeping rofi as the fallback.
3. **Polish from real use**: IME check, any ranking/look adjustments, then
   review and independent verification.
4. **Startup**: the quiet build measures 80/100 ms, level with the previous build
   and rofi's ~89 ms. Upstreaming the Rune patch is deferred; the resident daemon
   (ADR 0007) is optional work to go below the one-shot floor.

## Repository

Source and issues: <https://github.com/geril07/gofi>. Public since 2026-11-13.

## Blockers

None hard.

## Risks

- Cold start is measured under nested headless Sway. The client renders with the
  real GPU (RADV), while the harness compositor uses pixman; only presentation is
  software-composited. First frame is **80 ms median, 100 ms p95** with the quiet
  skin and startup patch. Rofi is ~89 ms to its first painted frame in the same
  harness, so they are level.
  The remaining fixed cost is Rune/wgpu's fresh-process GPU init and the
  compositor's first configure, not the provider. Re-measure on the owner's
  Swayfx session before promising the p95.
- The icons provider resolves every icon eagerly at startup, but the theme-chain
  index makes it ~20 ms regardless of app count. If that ever matters, resolve
  only the visible rows.
- IME/composed text input on Wayland needs a real test.
- Clipboard previews are sensitive; never log them or persist them. The provider
  streams decode → wl-copy. E2E screenshots contain only synthetic test strings.
- Rune is pre-release; API churn is expected. The revision is pinned and
  `scripts/check-rune.sh` enforces it and the local patches.
- Layer-shell immediate keyboard focus is not available; the ordinary-window path is
  the workaround (ADR 0003). With a bar, the workspace-centred panel sits below the
  bar rather than at the output's centre.

## Open questions for the owner

None. Resolved: `gofi`, MIT; three tabs (Apps / Run / Clipboard); `cliphist` as
the clipboard source; the 800×496 quiet panel; no metadata, footer or preview;
motion behind `GOFI_MOTION`; the window switcher stays out of MVP; no Sway config
changes by this project.

## References

- [Goals](goals.md)
- [Orchestration](orchestration.md)
- [Architecture](architecture.md)
- [Verification](verification.md)
- [Rune assessment](research/rune-assessment.md)
- [Current launcher](research/current-launcher.md)
- [Decisions](decisions/)
