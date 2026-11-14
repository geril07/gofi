# 0008 — Clipboard mode uses cliphist

- **Status:** accepted
- **Date:** 2026-11-10

## Context

Clipboard is in MVP. The owner already runs `wl-paste --watch cliphist store` in
Sway and binds `Mod+Ctrl+c` to:

```sh
cliphist list | rofi -dmenu | cliphist decode | wl-copy
```

`cliphist 0.7.0` stores up to 750 entries, emits `id<TAB>preview` from
`cliphist list`, and decodes an entry to stdout with `cliphist decode <id>`.
Reimplementing clipboard history would duplicate a working, battle-tested component
and add a new privacy surface.

## Decision

- Use `cliphist` as the clipboard source. gofi does not own a clipboard database.
- Clipboard mode loads `cliphist list` when the mode opens. Each line becomes an
  `Item` with the preview as its title and `clipboard:<id>` as a transient ID.
- Rank previews by nucleo match score desc, then first matched character asc, then
  the row's index in `cliphist list` (recency). Do not apply frecency and do not
  persist clipboard IDs.
- Selection runs `cliphist decode <id>` with stdout piped to `wl-copy` stdin,
  streaming the bytes. Then close the launcher.
- Show a generic clipboard icon; image thumbnails are out of MVP.
- Never log previews or decoded content. State contains only app/run frecency.
- The mode is reachable as `gofi clipboard` and through the Clipboard tab.

## Verification (2026-11-10)

With a private `-db-path` and a nested headless Sway, `cliphist store` → `list`
(id + preview) → `decode` → `wl-copy` → `wl-paste` returned the test value. The
same pipeline is what gofi will use; the remaining work is parsing and UI.

### Clipboard match ordering (2026-11-14)

The original decision also preserved cliphist's order when a query was typed, so
match quality never affected the list. That was an unargued default rather than a
constraint: sorting uses only the previews and list index already in memory, so it
adds no state and no new privacy surface.

The mode now sorts by match score desc, then the first matched character asc, then
the row's index in `cliphist list` (recency, newest first). The first two keys are
shared with Apps and Run (ADR 0004); the final key is the index because Clipboard
has no frecency and the preview is not a name. An empty query ties on both match
keys, so it stays pure recency.

Tradeoff: with a query, a strong older match now outranks a weak newer one. That is
what typing means, and it is why gofi exists, but the newest matching entry is no
longer always first.

fzf's `history` scheme (`--tiebreak=index`) is the other defensible choice; it
would drop the start-position key for this mode. gofi keeps it for consistency.

## Consequences

- gofi depends on `cliphist` and `wl-copy` being installed; both are already in the
  owner's environment.
- Clipboard history remains usable from rofi until gofi replaces the keybind.
- With a query, the newest matching entry is no longer always first; match quality
  wins. Empty-query output is unchanged.
- No image previews in MVP; the preview text from cliphist is enough to identify
  most entries.
- If cliphist changes its output format, only the clipboard provider changes.

## Revisit when

- The owner wants gofi to own clipboard history (dedupe rules, pins, image
  thumbnails).
- `cliphist` is removed or becomes unmaintained.
- Match-first ordering turns out to be worse in daily use than pure recency.
