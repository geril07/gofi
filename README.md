# gofi

A fast, native Wayland launcher for Sway. Applications, executables and clipboard
history, fuzzy matching, frecency, keyboard-first. Built in Rust on
[Rune](https://github.com/anomalyco/rune).

> Status: pre-alpha. The headless core and the Rune window work: apps, run and
> clipboard modes, icons, underlines, launch/copy, frecency. Sway integration and
> daily-use polish are next.

## Why

The current setup uses `rofi -show drun` and `rofi -show run`. The look is fine;
the sorting is not. gofi keeps the same muscle memory and adds a ranking model
that puts frequently and recently used items first without letting history
override a clearly better match.

## Build and run

Rune is a path dependency on the pinned clone in `references/rune` (the clone is
gitignored; `scripts/check-rune.sh` verifies the revision):

```sh
git clone https://github.com/anomalyco/rune references/rune
git -C references/rune checkout 04b38ae0a73bda03f010e97d2fff540ab91f67b0

just build
just install                   # copy to ~/.local/bin (BIN_DIR to override)
gofi                           # apps
./target/release/gofi run      # executables
./target/release/gofi clipboard

# Headless, no window:
./target/release/gofi --dump --mode apps --query fire
```

The launcher is a normal toplevel with `app_id="gofi"`. Sway gives it rofi's
geometry with this rule (not installed by the project):

```sway
for_window [app_id="gofi"] floating enable, border none, resize set 50 ppt 495 px, move position center
```

`move position center` follows the focused workspace, so the panel opens on the monitor
you are using. Do not use `move absolute position center`: with more than one output
it centres across the whole output layout instead. With a bar, the panel centres in
the workspace below it and sits slightly lower than rofi's output-centred layer-shell
surface. If your config sets `gaps inner N`, add `resize grow width N px` after
`resize set` to cancel the horizontal inset.

`just check` runs fmt, clippy and the tests; `just e2e` and `just e2e-launch` run
the nested-headless-Sway checks; `just measure` records startup.

## Keys

| Keys | Action |
|---|---|
| `Up` / `Down`, `Ctrl+P` / `Ctrl+N` | move the selection (wraps) |
| `PageUp` / `PageDown`, `Home` / `End` | jump a page / to the ends |
| `Tab` / `Shift+Tab`, `Shift+Right` / `Shift+Left` | switch mode |
| `Enter` | launch or copy |
| `Escape`, `Ctrl+G`, `Ctrl+C`, `Ctrl+[` | close |
| `Ctrl+A` / `Ctrl+E`, `Ctrl+B` / `Ctrl+F` | move the caret |
| `Ctrl+W` / `Ctrl+U` / `Ctrl+K` | clear / delete to start / delete to end |
| `Ctrl+H` / `Ctrl+D` | delete one character backward / forward |
| Wheel, click | move the selection, activate a row |

## Scope (MVP)

- `gofi` — application launcher (XDG `.desktop` entries, icons).
- `gofi run` — executable launcher (`$PATH`).
- `gofi clipboard` — cliphist picker; copies the selection with `wl-copy`.
- Fuzzy matching with smart case and Unicode normalization.
- Frecency: decayed use count, persisted per app/run mode. Clipboard keeps
  cliphist's recency order and stores nothing.
- Catppuccin Macchiato, JetBrains Mono, 32 px icons, seven 48 px rows,
  50% width × 495 px height (960×495 on 1920×1080), mode tabs (Apps / Run /
  Clipboard), bold and underlined match characters.
- Sway keybinds and a window rule; no X11, macOS or Windows.
- Window switcher is a future provider, not MVP.

## Docs

- [Goals](docs/goals.md)
- [Orchestration](docs/orchestration.md)
- [Architecture](docs/architecture.md)
- [Verification](docs/verification.md)
- [Status](docs/status.md)
- [Decisions](docs/decisions/)
- [Rune assessment](docs/research/rune-assessment.md)

## References

`references/` (gitignored) holds read-only clones used for behaviour and API study:

- `references/rune` — Rune at `04b38ae0a73bda03f010e97d2fff540ab91f67b0`.
- `references/rofi-src` — rofi 2.0.0 source.
- `references/rofi` — the user's current rofi config.
- `references/sway` — the user's current Sway config.
- `references/screenshots` — owner-provided rofi screenshots (private; gitignored).

## License

MIT. The embedded JetBrains Mono files are under the SIL Open Font License 1.1;
see `crates/gofi/assets/JetBrainsMono-OFL.txt`.
