# gofi

A fast, native Wayland launcher for Sway. Applications and executables, fuzzy
matching, frecency, keyboard-first. Built in Rust on [Rune](https://github.com/anomalyco/rune).

> Status: pre-alpha. No code yet. We are validating Rune and the Wayland surface
> strategy before scaffolding.

## Why

The current setup uses `rofi -show drun` and `rofi -show run`. The look is fine;
the sorting is not. gofi keeps the same muscle memory and adds a ranking model
that puts frequently and recently used items first without letting history
override a clearly better match.

## Scope (MVP)

- `gofi` — application launcher (XDG `.desktop` entries, icons).
- `gofi run` — executable launcher (`$PATH`).
- Fuzzy matching with smart case and Unicode normalization.
- Frecency: decayed use count, persisted per mode.
- Catppuccin Macchiato, JetBrains Mono, 32 px icons, seven rows, centred.
- Sway keybinds and a window rule; no X11, macOS or Windows.
- Window switcher and clipboard picker are future providers, not MVP.

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

## License

TBD (MIT unless the owner decides otherwise).
