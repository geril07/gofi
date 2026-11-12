# Status

**Last updated:** 2026-11-12
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
    Sway IPC parsing tests; **79 tests** total.
  - `just install` copies the release binary to `~/.local/bin/gofi`; the installed
    copy passes both nested-Sway e2e scripts. Sway config remains the owner's step.
  - Startup re-measured after caching the theme chain and directory lists:
    apps **median 300 ms, p95 330 ms** over 20 runs; still inside the 400 ms p95
    target. The first two runs were 140–180 ms (warm page cache).

## In progress

Nothing. **M4 — Sway integration and daily use — is next.**

## Next (ordered)

1. **Sway integration**: gofi is installed at `~/.local/bin/gofi`; the owner adds
   the window-rule snippet from the README and points the keybinds at it. No config
   was changed by this project.
2. **Daily-use checklist** for one week, keeping rofi as the fallback.
3. **Polish from real use**: IME check, any ranking/look adjustments, then
   review and independent verification.

## Blockers

None hard.

## Risks

- Cold start is measured under nested headless Sway. The client renders with the
  real GPU (RADV), while the harness compositor uses pixman; only presentation is
  software-composited. First frame is ~126 ms (rofi 58 ms); the ~100 ms UI slice
  is Rune/wgpu init and the first draw, not icons. Re-measure on the owner's
  Swayfx session before promising the p95.
- The icons provider resolves every icon eagerly at startup, but the theme-chain
  index makes it ~20 ms regardless of app count. If that ever matters, resolve
  only the visible rows.
- IME/composed text input on Wayland needs a real test.
- Clipboard previews are sensitive; never log them or persist them. The provider
  streams decode → wl-copy. E2E screenshots contain only synthetic test strings.
- Rune is pre-release; API churn is expected. The revision is pinned and
  `scripts/check-rune.sh` enforces it.
- Layer-shell immediate keyboard focus is not available; the ordinary-window path is
  the workaround (ADR 0003). The window is 1 px above rofi's y on a 1080p output.

## Open questions for the owner

None. Resolved: `gofi`, MIT; three tabs (Apps / Run / Clipboard); `cliphist` as
the clipboard source; the 50% × 495 geometry; the window switcher stays out of
MVP; no Sway config changes by this project.

## References

- [Goals](goals.md)
- [Orchestration](orchestration.md)
- [Architecture](architecture.md)
- [Verification](verification.md)
- [Rune assessment](research/rune-assessment.md)
- [Current launcher](research/current-launcher.md)
- [Decisions](decisions/)
