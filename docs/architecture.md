# Architecture

This is the working shape, not a frozen spec. It exists so workers can make local
decisions without inventing a different structure each time. Changes that affect the
provider boundary, ranking, state format or process model need an ADR.

## Data flow

```
CLI (mode, query)
   │
   ▼
Mode registry ──► Provider::load() ──► Vec<Item>
   │                                   │
   │                                   ▼
   │                            Matcher (nucleo)
   │                                   │
   │                                   ▼
   │                            Ranker (frecency or recency)
   │                                   │
   │                                   ▼
   │                            ViewModel (rows, selection)
   │                                   │
   │                                   ▼
   │                            Rune view / headless tree
   │                                   │
   ▼                                   ▼
Action ◄────────────────────────── Enter / click
   │
   ├─ LaunchArgv ──► detached spawn ──► frecency update
   └─ CopyClipboard ──► cliphist decode ──► wl-copy
```

The UI knows nothing about `.desktop` files, `$PATH`, `swaymsg` or clipboard
formats. It renders `Item`s and emits an `Action`. That is the extensibility seam.

## Modules

```
crates/
  gofi-core/                 no UI dependency; fast headless tests
    src/
      lib.rs
      model.rs               Item, ItemId, Mode, Action, Provider, Loaded
      xdg.rs                 XDG dirs, desktop env, locale, test Env
      providers/
        apps.rs              XDG .desktop discovery and parsing
        run.rs               $PATH executable discovery
        clipboard.rs         cliphist list, no persisted history
      ranking/
        mod.rs               nucleo pattern + composite sort
        frecency.rs          decay math and normalization
      store.rs               state.json load/save, atomic write, lock
      icons.rs               icon name → file path resolution
    tests/
      providers.rs
      ranking_state.rs
  gofi/                      the binary and the Rune window
    src/
      main.rs                CLI parsing, --dump, window startup
      ui.rs                  app state, view, keys, pointer, scroll
      ui/tests.rs            headless Runtime tests (geometry, rows, keys)
      launch.rs              detached spawn, cliphist decode → wl-copy
      instance.rs            flock single instance, SIGTERM toggle-close
```

The UI crate depends on `gofi-core` and Rune. `gofi-core` stays UI-free so the
provider/ranking/state tests run in milliseconds.

Keep modules small. A file over ~500 lines is a signal to split by capability, not a
hard rule. Tests live beside the behavior they cover or in `tests/` when they need a
real temp tree.

## Provider boundary

```rust
pub trait Provider {
    fn load(&self) -> Result<Loaded, ProviderError>;
}

pub struct Item {
    pub id: ItemId,          // stable across runs; frecency key
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<PathBuf>, // resolved file path; the UI only decodes it
    pub search_text: String,   // title + generic name + keywords, original case
    pub kind: ItemKind,
    pub action: Action,
}
```

- `ItemId` is `apps:<desktop-file-id>`, `run:exec:<name>` or `clipboard:<id>`. Only
  the first two are frecency keys; clipboard IDs are never persisted.
- `Action` is an enum (`Launch { argv, cwd }`, `CopyClipboard { id }`, later
  `FocusWindow`, ...). The UI executes it; providers build argv and never shell out.
- `Loaded` carries items plus per-entry warnings, so one bad file does not fail the
  mode.
- `Provider::load` is synchronous for MVP. If a scan is slow, the provider can expose
  a background refresh later without changing the UI; the UI already renders a list
  that can be replaced.

## Ranking

Use `nucleo-matcher` (Helix's matcher) for fuzzy scoring:

- `CaseMatching::Smart`, `Normalization::Smart`.
- Pattern parsed once per query; matcher reused; no per-item allocation beyond the
  scratch buffer.
- Score is the matcher's score; do not reimplement its bonuses.

Composite ordering:

1. Filter out non-matches.
2. `top = max(match_score)`.
3. `frecency_norm = score / (score + K)` with `K ≈ 10` (saturating).
4. `composite = match_score + 0.15 * top * frecency_norm`.
5. Sort by `composite` desc, then title asc.

Empty query: `composite = frecency_norm`, then title. This lets a frequent item
break ties and promote a slightly worse match, but a clearly better match wins.
The 0.15 factor and K are tuning constants; the fixed corpus test pins the intended
ordering, not the exact numbers.

Clipboard mode is the exception: filter by fuzzy match, but keep cliphist's recency
order. Do not apply frecency, do not persist clipboard IDs, and do not reorder by
score. The most recent matching entry is usually the intended one.

## Frecency

Each item stores `score: f64` and `last_used: u64` (Unix seconds). On use:

```
decayed = score * 0.5^((now - last_used) / HALF_LIFE)
score = decayed + 1.0
last_used = now
```

`HALF_LIFE = 14 days`. On read, apply the same decay so an unused item fades without
a write. This captures frequency and recency in O(1) space per item, with no history
list to compact. The constants are easy to tune; the behavior test uses a fake clock.

## State

`$XDG_STATE_HOME/gofi/state.json` (fallback `~/.local/state/gofi/state.json`):

```json
{
  "version": 1,
  "modes": {
    "apps": { "apps:firefox.desktop": { "score": 3.2, "last_used": 1760000000 } },
    "run":  { "run:exec:code":       { "score": 1.0, "last_used": 1760000000 } }
  }
}
```

- Load: missing → empty; corrupt → empty + warning; unknown version → empty + warning.
- Save: write `state.json.tmp`, `fsync`, rename; take an advisory lock so two
  launcher instances cannot lose an update.
- Bound the number of entries per mode (e.g. 2,000) so the file cannot grow without
  limit. Evict the lowest score when over the cap.

No database. The data is small, transparent and easy to test. Revisit if a future
provider needs queryable history.

Clipboard entries are deliberately absent from this file. `cliphist` owns clipboard
history; gofi only reads it and copies a selection. Never log previews or decoded
content.

## Clipboard mode

`cliphist` is already running in the owner's session (`wl-paste --watch cliphist
store`). Use it as the source of truth:

- `cliphist list` → lines `id<TAB>preview` (preview width 100 by default, max 750
  entries). Parse at the first tab; keep the preview as the row title.
- Filter previews with the same nucleo matcher, but keep cliphist order (most recent
  first). Do not apply frecency.
- `Action::CopyClipboard { id }` runs `cliphist decode <id>` with stdout piped into
  `wl-copy` stdin. Stream the bytes; do not buffer a large image if it can be avoided.
- Show a generic clipboard icon; image thumbnails are out of MVP.
- `cliphist`/`wl-copy` missing or failing → an error row or a log + notification,
  never a panic.
- The mode is reachable by `gofi clipboard` and by the Clipboard tab. Switching modes
  clears the query and resets the selection.

This is the first non-app provider and validates the `Provider`/`Action` boundary.

## Launching

- Build `argv: Vec<OsString>` from the parsed `Exec` line at discovery time (or
  lazily, but never via a shell).
- Field codes: remove `%f/%F/%u/%U` when no file arguments; expand `%i`, `%c`,
  `%k`; `%%` → `%`. Reject unknown codes with a warning.
- `Terminal=true`: prepend `$TERMINAL`, else `kitty`, else `xterm`.
- `Path=` sets the working directory if it exists.
- Spawn with stdin/stdout/stderr null, `process_group(0)` so the child survives the
  launcher. Close the launcher first; log spawn failures and optionally send a
  desktop notification.
- Never `sh -c` a desktop entry. Never interpolate the query into a command.

## Icons

- `Icon=` absolute path → use it.
- Otherwise follow GTK's theme lookup: the `gtk-icon-theme-name` from
  `~/.config/gtk-3.0`/`gtk-4.0/settings.ini` (default `Adwaita`), that theme's
  `Inherits` chain, then `hicolor`. Each theme's `Directories=` list is searched
  with sizes nearest 32 px first (`32x32`, `48x48`, … `16x16`, `scalable`), so
  legacy categories such as `16x16/legacy` resolve too. The quick path probes
  `apps/` only; the full walk runs on a miss. Rune rasterizes SVG at the draw size
  (the `svg` feature).
- Path resolution caches the theme chain and each theme's directory list per
  process; Rune's `ImageCache` caches decoded pixels. The UI keeps one `Image`
  handle per (path, display scale).
- Missing icon → an empty 32 px slot so app rows stay aligned; never fail the row.
  Run rows have no icon; clipboard rows draw a generic glyph.

## Window geometry

The launcher is a normal toplevel; Sway owns its placement and size. The rule
`for_window [app_id="gofi"] floating enable, border none, move position center, resize set 50 ppt 495 px`
matches rofi's layer-shell geometry on both the 1920×1080 and 2560×1600 @ 1.5
outputs (verified). The app does not need to query display sizes or position itself
on Wayland; it should use a sane initial size and let the rule win. On another
compositor, the equivalent rule/wrapper is the integration point.

## UI (M2)

One Rune view, no scroll container: the visible slice is seven rows, so the view
renders only those. Geometry is the measured rofi layout at a 960×495 panel:

| Element | Geometry |
|---|---|
| Panel | full window, mantle `#1e2030`, no radius (the compositor may round it) |
| Top row | y 10, height 59, 20 px side inset |
| Search field | left, 16 px inner padding, placeholder `Search...` |
| Tabs | right-aligned, three 100×59 buttons, 20 px gap, radius 8 |
| List | y 89, x 20, width `panel - 58`, seven 48 px rows, 9 px gap |
| Row | radius 8; selected blue `#8aadf4` with mantle text |
| App icon | 32×32 at 9 px from the row's left edge; title at 57 px |
| Run row | no icon; title at 15 px |
| Scrollbar | 8 px at the right, base track, blue handle, rofi's length formula |

The list follows rofi's `scroll-method: 1`: the selected row walks down to the
middle of the seven rows, then the list follows it (`offset = selected - 3`); near
the end the list stops at a full seven-row page, and a short list keeps the gap
below. The scrollbar handle is positioned absolutely inside its track and tracks
the selected row, as rofi's does; it is not the viewport offset.

Keys are handled by the focused text input first: `Up`/`Down` and `Ctrl+P`/`Ctrl+N`
wrap the selection, `PageUp`/`PageDown` move seven rows, `Home`/`End` jump to the
ends, `Tab`/`Shift+Tab` and `Shift+Right`/`Shift+Left` cycle modes, `Enter` (also
`Ctrl+J`/`Ctrl+M`) activates, and `Escape`/`Ctrl+G`/`Ctrl+C`/`Ctrl+[` close.
`Ctrl+A`/`Ctrl+E` move the caret to the ends, `Ctrl+B`/`Ctrl+F` one character,
`Ctrl+W` clears the line, `Ctrl+U`/`Ctrl+K` delete to the start/end, and
`Ctrl+H`/`Ctrl+D` delete one character backward/forward.
Wheel over the list moves the selection one row per event, with rofi's direction
(`reverse: false`: wheel down moves down). A click activates the row;
`focus_on_window_activation` and `on_focus(false)` close the window.

Match underlines come from `Ranked::positions`, which are **character** indices into
the title; the view maps them to UTF-8 byte ranges and merges adjacent characters
into one `Span::underline` span. A keyword-only match has no positions, so it has no
underline.

Mode loading is lazy: the CLI mode loads at startup; switching to a mode that has not
been loaded runs that provider once (apps ~30 ms, run ~14 ms, clipboard ~15 ms) and
keeps the items for the rest of the process. Clipboard IDs and previews never enter
the frecency state.

## Process model (one-shot accepted)

A one-shot process: Sway keybind runs `gofi`, window opens, selection launches and
exits. This is simple and matches rofi. Release `hello` measured 90 ms median
readiness (240 ms first run); the real app must be measured again.

Single instance (ADR 0009): the process holds `$XDG_RUNTIME_DIR/gofi.lock` with
`flock`. A second invocation reads the holder's PID and sends `SIGTERM` if
`/proc/<pid>/comm` is `gofi`, which is the accepted "press the keybind again to close
it" behavior. `--dump` never takes the lock.

If cold start misses the budget, the resident fallback is:

- `gofi toggle` connects to `$XDG_RUNTIME_DIR/gofi.sock`; if absent, spawns
  `gofi daemon --show` detached.
- The daemon keeps the GPU device and parsed providers warm, but **destroys the
  window on close and recreates it on toggle**. On Wayland a recreated surface is a
  new window, so the compositor grants focus without an activation-token dance.
- The daemon owns the state file; the client only sends `toggle` / `quit`.

Do not build the daemon before measuring. Record the decision in ADR 0007.

## Error handling

- Provider errors are per-entry: one bad `.desktop` file does not hide the rest.
- Startup errors (state unreadable, no XDG dirs) log and continue with an empty list.
- Launch errors are reported to the user (log + notification) and do not crash.
- Clipboard errors are reported without printing the entry content.
- No `unwrap()` on filesystem or user input in production paths; tests may use it.

## Performance notes

- Parse desktop entries once at startup; a few hundred files is cheap. If measured
  otherwise, cache by mtime or move to a background refresh.
- PATH scan: read directories, check metadata, dedupe by name. Do not canonicalize
  every path; `metadata` follows symlinks and is enough.
- Clipboard: `cliphist list` is capped at 750 entries and is fast; run it when the
  mode opens, not on every keystroke. Filtering previews is the same matcher path.
- Ranking: score only when the query changes; reuse the matcher and scratch buffers.
  Do not allocate a `String` per item per keystroke.
- UI: build only visible rows (`virtual_rows` if the list is long); memoize rows that
  do not change. The MVP can build all rows for a few thousand items if measured OK.
- Do not redraw while hidden. Return `false` from `poll` when nothing changed.
