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
./target/release/gofi          # apps
./target/release/gofi run      # executables
./target/release/gofi clipboard

# Headless, no window:
./target/release/gofi --dump --mode apps --query fire
```

The launcher is a normal toplevel with `app_id="gofi"`. Sway gives it rofi's
geometry with this rule (not installed by the project):

```sway
for_window [app_id="gofi"] floating enable, border none, move position center, resize set 50 ppt 495 px
```

`just check` runs fmt, clippy and the tests; `just e2e` and `just e2e-launch` run
the nested-headless-Sway checks; `just measure` records startup.

## Scope (MVP)

- `gofi` — application launcher (XDG `.desktop` entries, icons).
- `gofi run` — executable launcher (`$PATH`).
- `gofi clipboard` — cliphist picker; copies the selection with `wl-copy`.
- Fuzzy matching with smart case and Unicode normalization.
- Frecency: decayed use count, persisted per app/run mode. Clipboard keeps
  cliphist's recency order and stores nothing.
- Catppuccin Macchiato, JetBrains Mono, 32 px icons, seven 48 px rows,
  50% width × 495 px height (960×495 on 1920×1080), mode tabs (Apps / Run /
  Clipboard), matched-character underlines.
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
