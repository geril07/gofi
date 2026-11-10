# Goals

## One line

**gofi** is a fast, native Wayland launcher for Sway that replaces `rofi -show drun`
and `rofi -show run` with better fuzzy ranking and frecency, built in Rust on
[Rune](https://github.com/anomalyco/rune).

Working title. The owner may rename it; do not spend design time on branding yet.

## Why

The current setup works but the owner dislikes the sorting: rofi does not put
frequently used applications first in a way that feels right. The launcher is used
constantly, so small ranking improvements are felt every day. Building it is also a
chance to evaluate Rune as a native UI framework and to have fun.

The visual experience is already good. The goal is **behavioural parity with the
current launcher, plus a ranking model that is clearly better**, not a redesign for
its own sake.

## User and environment (verified)

- One user: `geril`, on Sway (`swayfx 0.6`, based on Sway 1.12), native Wayland.
- Current launcher: `rofi 2.0.0` (Arch package), Catppuccin Macchiato theme,
  JetBrains Mono 16, 32 px icons, seven list rows, centred.
- Current keybinds in `~/.config/sway/config`:
  - `Mod+Return` → `rofi -show drun` (applications)
  - `Mod+Shift+Return` → `rofi -show run` (executables)
  - `Mod+Shift+w` → `rofi -show window` (window switcher; **future**)
  - `Mod+Ctrl+c` → `cliphist list | rofi -dmenu | cliphist decode | wl-copy`
    (clipboard; **future**)
- Terminal: kitty. Font: JetBrains Mono. Theme: Catppuccin Macchiato.
- Existing rofi config is copied to `references/rofi/`; Sway config to
  `references/sway/`.

## MVP scope

### Modes

1. **Applications** (`gofi`, default): discover XDG `.desktop` entries, show icon,
   name and optional generic name, launch the parsed `Exec` argv.
2. **Executables** (`gofi run`): scan `$PATH`, deduplicate by name, launch the first
   match in `PATH` order.

The two modes preserve the current muscle memory. A combined list and a mode
switcher are allowed later, but the two Sway keybinds must keep working.

### Matching and ranking

- Fuzzy matching with smart case and Unicode normalization.
- Scoring that rewards exact, prefix, word-boundary and camel-case matches and
  penalizes gaps (use a proven matcher, not a hand-rolled Smith-Waterman).
- Frecency: decayed use count, per mode, persisted across restarts.
- Empty query: most frecent first, then alphabetical.
- Frecency may promote a slightly worse match, but must not override a clearly
  better one. The exact blend is an ADR; tune with a fixed corpus of real queries.

### Interaction (parity with current rofi)

- Type to filter; results update on every keystroke.
- `Up`/`Down`, `Ctrl+P`/`Ctrl+N`, `PageUp`/`PageDown`, `Home`/`End`.
- `Enter` launches; `Escape` closes; focus loss closes.
- `Tab`/`Shift+Tab` switches mode if a combined mode is added.
- Click a row to launch; scroll moves the selection.
- Selected row resets to the top on query change; list wraps at the ends.
- Query is empty and focused on open; the previous query is not restored by default.

### Look

Match the current rofi theme closely enough that the switch is invisible:

- Catppuccin Macchiato: mantle window, blue selection, base text on selection,
  text `#cad3f5`, blue `#8aadf4`.
- JetBrains Mono 16 (Rune may fall back to its bundled font if the system font is
  unavailable; do not bundle the font unless licensing is clear).
- 32 px icons, 7 visible rows, 8 px row radius, generous row padding.
- Centred, borderless, no titlebar; width ~700 logical px; height fits 7 rows.
- No window shadow from the compositor if Swayfx adds one; the panel draws its own
  rounded background.

### Performance

Targets, measured on the owner's machine (not asserted in unit tests):

- Cold start to interactive window: **≤ 400 ms p95**.
- Warm toggle (resident process, if adopted): **≤ 80 ms p95**.
- Query update with ~5,000 executables: **≤ 16 ms p95** on the UI thread.
- Idle: no redraws while the window is hidden; negligible CPU.

If cold start cannot meet the target, the fallback is a resident daemon with a
prewarmed GPU and a recreated window per toggle (see ADR 0007). Measure before
choosing.

### State

- `$XDG_STATE_HOME/gofi/state.json` (fallback `~/.local/state/gofi/state.json`).
- Per-mode frecency keyed by stable item ID (`desktop-file-id` for apps,
  `exec:<name>` for run).
- Atomic writes; corrupt/missing state falls back to empty with a warning, never a
  crash.

### Sway integration

- `rune::set_app_id("gofi")` so window rules match `app_id="gofi"`.
- Window rule: floating, borderless, centred, correct size; does not affect other
  windows.
- Two keybinds replacing the rofi ones only after the daily-use checklist passes.
- Keep the rofi keybinds commented in the config until then; do not break the
  owner's daily driver during development.

## Out of scope for MVP

- Window switcher (`rofi -show window`), clipboard history, file browser, SSH,
  calculator, web search, custom script modes.
- Plugins or a scripting API.
- X11, macOS, Windows, mobile.
- GUI settings, theme editor, per-app rules.
- Multi-user/system packaging; AUR/Flatpak can come later.
- Pixel-perfect golden tests; visual polish is reviewed by screenshots.

The architecture must leave room for future providers (window switcher, clipboard,
custom pickers) without rewriting the list/ranking/launch core. That means a
`Provider` boundary and an `Action` boundary, not a plugin system.

## Success criteria

1. The owner uses gofi for apps and executables for **one week** without falling back
   to rofi.
2. Typing 2–3 letters finds the intended item in the top three; frequent apps appear
   first on an empty query.
3. Cold/warm performance meets the targets above.
4. The launcher never steals focus unexpectedly, never appears during tests, and
   closes on focus loss.
5. `cargo test`, `clippy -D warnings`, and the nested-Sway e2e pass from a clean
   checkout.
6. A new contributor (or agent) can read `docs/` and reproduce the build, test and
   verification without asking the owner.

## Milestones

- [ ] **M0 — validation and infrastructure**
  - [x] Rune builds on Linux with the pinned toolchain (Rust 1.99.1).
  - [x] Rune renders a window under nested headless Sway (wgpu + pixman backend).
  - [x] Current rofi/Sway configs copied to `references/`.
  - [x] Measure Rune cold start on this machine (release `ready_ms`: 90 ms median,
        240 ms first run; compositor presentation not included).
  - [x] Prove text input, key handling and focus-loss close in a real Rune window.
  - [x] Prove Sway window rule (`app_id="gofi"`) floats/centres the window.
  - [x] Nested-Sway harness script with isolated env and evidence
        (`scripts/e2e-sway.sh`, verified with the surface spike).
- [ ] **M1 — core model, no UI**
  - Desktop entry parser + discovery with tests.
  - PATH executable scan with tests.
  - Nucleo ranking + frecency with a fixed corpus and tests.
  - State load/save with corruption and atomicity tests.
  - `gofi --dump --mode apps|run --query ...` JSON output for headless checks.
- [ ] **M2 — Rune UI**
  - Window, theme, input, list, icons, selection, scrolling.
  - Headless `--snap`/`--tree` scenes.
  - Keyboard/mouse parity checks.
- [ ] **M3 — launch + state**
  - `Exec` argv building, terminal wrapping, detached spawn.
  - Frecency update on launch; persistence.
  - Failure reporting (log/notification) without blocking.
- [ ] **M4 — Sway integration + performance**
  - Window rule and keybinds; focus-loss behavior.
  - Cold/warm measurements; daemon decision.
  - Daily-use checklist.
- [ ] **M5 — polish and handoff**
  - Config/CLI docs, install instructions, review + independent verification.
  - Optional: replace rofi in the owner's Sway config.

## Open questions for the owner

1. **Screenshots** of the current rofi (apps and run modes) to match spacing, width,
   row height and mode indicator. The config gives most values; a screenshot settles
   the rest.
2. **Two modes vs one combined list.** Default plan: two modes, preserving the two
   keybinds; Tab switches if a combined mode is added later. Confirm.
3. **Resident daemon.** Acceptable to keep one background process for instant toggles?
   Default plan: measure cold start first; adopt the daemon only if cold start misses
   the target.
4. **Name and license.** Keep `gofi` and MIT, or choose another name/license?
5. **Window switcher / clipboard.** Confirm they stay out of MVP even though the
   architecture leaves room.
