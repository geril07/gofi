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
- Fuzzy-filter previews, but preserve cliphist's recency order. Do not apply
  frecency and do not persist clipboard IDs.
- Selection runs `cliphist decode <id>` with stdout piped to `wl-copy` stdin,
  streaming the bytes. Then close the launcher.
- Show a generic clipboard icon; image thumbnails are out of MVP.
- Never log previews or decoded content. State contains only app/run frecency.
- The mode is reachable as `gofi clipboard` and through the Clipboard tab.

## Verification (2026-11-10)

With a private `-db-path` and a nested headless Sway, `cliphist store` → `list`
(id + preview) → `decode` → `wl-copy` → `wl-paste` returned the test value. The
same pipeline is what gofi will use; the remaining work is parsing and UI.

## Consequences

- gofi depends on `cliphist` and `wl-copy` being installed; both are already in the
  owner's environment.
- Clipboard history remains usable from rofi until gofi replaces the keybind.
- No image previews in MVP; the preview text from cliphist is enough to identify
  most entries.
- If cliphist changes its output format, only the clipboard provider changes.

## Revisit when

- The owner wants gofi to own clipboard history (dedupe rules, pins, image
  thumbnails).
- `cliphist` is removed or becomes unmaintained.
- Clipboard entries need ranking beyond recency.
