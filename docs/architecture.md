# Architecture (proposed)

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
src/
  main.rs            CLI parsing, Rune app wiring, headless dump mode
  app.rs             app state, view, key/mouse handling, selection
  model.rs           Item, ItemId, Mode, Action, SourceKind
  providers/
    mod.rs           Provider trait + registry
    apps.rs          XDG .desktop discovery and parsing
    run.rs           $PATH executable discovery
    clipboard.rs     cliphist list/decode, no persisted history
  ranking/
    mod.rs           nucleo pattern + composite sort
    frecency.rs      decay math and normalization
  store.rs           state.json load/save, atomic write, lock
  launch.rs          Exec argv building, terminal wrapping, detached spawn
  icons.rs           icon name → file path resolution
  xdg.rs             XDG base dirs, desktop env, locale
tests/
  fixtures/          .desktop files, fake PATH trees, state files
  *.rs               integration tests for providers/ranking/store/launch
```

Keep modules small. A file over ~500 lines is a signal to split by capability, not a
hard rule. Tests live beside the behavior they cover or in `tests/` when they need a
real temp tree.

## Provider boundary

```rust
pub trait Provider {
    fn load(&self) -> Result<Vec<Item>, ProviderError>;
}

pub struct Item {
    pub id: ItemId,          // stable across runs; frecency key
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<IconSource>,
    pub search_text: String, // title + keywords + exec, lowercased where useful
    pub action: Action,
}
```

- `ItemId` is `apps:<desktop-file-id>`, `run:exec:<name>` or `clipboard:<id>`. Only
  the first two are frecency keys; clipboard IDs are never persisted.
- `Action` is an enum (`LaunchArgv`, `CopyClipboard`, later `FocusWindow`, ...). The
  UI calls `execute(action, &mut Frecency)`; clipboard execution does not touch
  frecency.
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
    "apps": { "firefox.desktop": { "score": 3.2, "last_used": 1760000000 } },
    "run":  { "exec:code":      { "score": 1.0, "last_used": 1760000000 } }
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
- Otherwise search the current icon theme, `hicolor`, `Adwaita`, and
  `/usr/share/pixmaps` for `{name}.png`, `{name}.svg`, `{name}.xpm` (Rune cannot
  decode XPM; skip it) at 32/48/64/128/scalable.
- Cache path resolution per icon name; let Rune's `ImageCache` cache decoded pixels.
- Missing icon → a neutral fallback glyph; never fail the row.

## Process model (one-shot accepted)

A one-shot process: Sway keybind runs `gofi`, window opens, selection launches and
exits. This is simple and matches rofi. Release `hello` measured 90 ms median
readiness (240 ms first run); the real app must be measured again.

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
