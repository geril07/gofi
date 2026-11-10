# Goals

## One line

**gofi** is a fast, native Wayland launcher for Sway that replaces `rofi -show drun`,
`rofi -show run` and the rofi/cliphist clipboard picker with better fuzzy ranking and
frecency, built in Rust on [Rune](https://github.com/anomalyco/rune).

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
    (clipboard; **MVP**) → `gofi clipboard`
- Terminal: kitty. Font: JetBrains Mono. Theme: Catppuccin Macchiato.
- Existing rofi config is copied to `references/rofi/`; Sway config to
  `references/sway/`.

## MVP scope

### Modes

1. **Applications** (`gofi`, default): discover XDG `.desktop` entries, show icon,
   name and optional generic name, launch the parsed `Exec` argv.
2. **Executables** (`gofi run`): scan `$PATH`, deduplicate by name, launch the first
   match in `PATH` order.
3. **Clipboard** (`gofi clipboard`): list `cliphist` entries, fuzzy-filter the
   previews, and copy the selected entry back with `cliphist decode | wl-copy`.
   No own clipboard database; cliphist remains the source of truth.

The three modes preserve the current muscle memory. The launcher exposes a mode
switcher matching rofi's tabs: Apps (magnifier), Run (`</>`), Clipboard
(clipboard icon). The folder/filebrowser tab from rofi is dropped. The same modes
are also reachable by CLI, so the tab is a convenience, not the only path.

### Matching and ranking

- Fuzzy matching with smart case and Unicode normalization.
- Scoring that rewards exact, prefix, word-boundary and camel-case matches and
  penalizes gaps (use a proven matcher, not a hand-rolled Smith-Waterman).
- Frecency: decayed use count, per mode, persisted across restarts.
- Empty query: most frecent first, then alphabetical.
- Frecency may promote a slightly worse match, but must not override a clearly
  better one. The exact blend is an ADR; tune with a fixed corpus of real queries.
- Clipboard mode is the exception: it uses cliphist's recency order, fuzzy-filters
  the previews, and does **not** persist or rank by frecency. Clipboard contents are
  sensitive; keep them out of state and logs.

### Interaction (parity with current rofi)

- Type to filter; results update on every keystroke.
- `Up`/`Down`, `Ctrl+P`/`Ctrl+N`, `PageUp`/`PageDown`, `Home`/`End`.
- `Enter` launches/copies; `Escape` closes; focus loss closes.
- `Tab`/`Shift+Tab` switches modes (Apps → Run → Clipboard).
- Click a row to launch/copy; click a tab to switch mode; scroll moves the selection.
- Selected row resets to the top on query change; list wraps at the ends.
- Query is empty and focused on open; the previous query is not restored by default.
- Clipboard selection copies the entry and closes; do not paste automatically.

### Look

Match the current rofi theme closely enough that the switch is invisible:

- Catppuccin Macchiato: mantle window, blue selection, base text on selection,
  text `#cad3f5`, blue `#8aadf4`.
- JetBrains Mono 16 (Rune may fall back to its bundled font if the system font is
  unavailable; do not bundle the font unless licensing is clear).
- 32 px icons, 7 visible rows, 48 px row height, 57 px pitch, 8 px row radius.
- Centred, borderless, no titlebar; **50% of the output width × 495 logical px
  height**. On the 1920×1080 output that is 960×495; on the 2560×1600 @ 1.5 laptop
  output it is 853×495 logical (1279×742 physical). Verified against the real rofi
  under the owner's config in nested Sway.
- Top bar: search input on the left, three mode tabs (100×59, 8 px radius, 21 px
  gap) on the right. Selected tab is blue; unselected tabs are base.
- Matched query characters are underlined in both selected and unselected rows.
- Selected row: blue `#8aadf4`, mantle text. Unselected: text `#cad3f5`.
- Scrollbar: 8 px handle, blue, at the right edge.
- No window shadow from the compositor if Swayfx adds one; the panel draws its own
  rounded background.

### Performance

Targets, measured on the owner's machine (not asserted in unit tests):

- Cold start to interactive window: **≤ 400 ms p95**.
- Warm toggle (resident process, if adopted): **≤ 80 ms p95**.
- Query update with ~5,000 executables: **≤ 16 ms p95** on the UI thread.
- Clipboard mode open with 750 cliphist entries: **≤ 100 ms** to first results.
- Idle: no redraws while the window is hidden; negligible CPU.

If cold start cannot meet the target, the fallback is a resident daemon with a
prewarmed GPU and a recreated window per toggle (see ADR 0007). Measure before
choosing.

### State

- `$XDG_STATE_HOME/gofi/state.json` (fallback `~/.local/state/gofi/state.json`).
- Per-mode frecency keyed by stable item ID (`desktop-file-id` for apps,
  `exec:<name>` for run).
- Clipboard entries are **not** persisted; cliphist owns that history. Never write
  clipboard contents or IDs to gofi state or logs.
- Atomic writes; corrupt/missing state falls back to empty with a warning, never a
  crash.

### Sway integration

- `rune::set_app_id("gofi")` so window rules match `app_id="gofi"`.
- Window rule: floating, borderless, centred,
  `resize set 50 ppt 495 px`; does not affect other windows. This exactly matches
  rofi's geometry on both the 1920×1080 and 2560×1600 @ 1.5 outputs.
- Keybinds replacing the rofi ones only after the daily-use checklist passes:
  - `Mod+Return` → `gofi`
  - `Mod+Shift+Return` → `gofi run`
  - `Mod+Ctrl+c` → `gofi clipboard`
- Keep the rofi keybinds commented in the config until then; do not break the
  owner's daily driver during development.

## Out of scope for MVP

- Window switcher (`rofi -show window`), file browser, SSH, calculator, web search,
  custom script modes.
- Plugins or a scripting API.
- X11, macOS, Windows, mobile.
- GUI settings, theme editor, per-app rules.
- Multi-user/system packaging; AUR/Flatpak can come later.
- Pixel-perfect golden tests; visual polish is reviewed by screenshots.

The architecture must leave room for future providers (window switcher, custom
pickers) without rewriting the list/ranking/launch core. That means a `Provider`
boundary and an `Action` boundary, not a plugin system. Clipboard is the first
non-app provider and validates that boundary.

## Success criteria

1. The owner uses gofi for apps, executables and clipboard for **one week** without
   falling back to rofi.
2. Typing 2–3 letters finds the intended item in the top three; frequent apps appear
   first on an empty query.
3. Cold/warm performance meets the targets above.
4. The launcher never steals focus unexpectedly, never appears during tests, and
   closes on focus loss.
5. Clipboard selection copies the right entry and never writes clipboard contents to
   gofi state or logs.
6. `cargo test`, `clippy -D warnings`, and the nested-Sway e2e pass from a clean
   checkout.
7. A new contributor (or agent) can read `docs/` and reproduce the build, test and
   verification without asking the owner.

## Milestones

- [x] **M0 — validation and infrastructure**
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
  - Cliphist list/decode provider with tests (fake `cliphist`/`wl-copy`).
  - Nucleo ranking + frecency with a fixed corpus and tests.
  - State load/save with corruption and atomicity tests.
  - `gofi --dump --mode apps|run|clipboard --query ...` JSON output for headless
    checks.
- [ ] **M2 — Rune UI**
  - Window, theme, input, list, icons, selection, scrolling.
  - Mode tabs (Apps / Run / Clipboard) and match underlines.
  - Headless `--snap`/`--tree` scenes.
  - Keyboard/mouse parity checks.
- [ ] **M3 — launch + state**
  - `Exec` argv building, terminal wrapping, detached spawn.
  - Clipboard copy pipeline (`cliphist decode` → `wl-copy`).
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

1. **Name and license.** Keep `gofi` and MIT, or choose another name/license?
2. **Window switcher.** Confirm it stays out of MVP even though the architecture
   leaves room for it.

Resolved: three tabs (Apps / Run / Clipboard), `cliphist` as the clipboard source,
and the 50% width × 495 logical height geometry (the Sway rule
`resize set 50 ppt 495 px`) verified against the real config on both outputs.
