# gofi

[![CI](https://github.com/geril07/gofi/actions/workflows/ci.yml/badge.svg)](https://github.com/geril07/gofi/actions/workflows/ci.yml)

<video src="https://github.com/user-attachments/assets/a0cbf96d-a24f-4233-90c7-df2ae20030f2" controls muted></video>

<sub>[Still](docs/assets/launcher.png) · [download the MP4](docs/assets/demo.mp4) · demo data is synthetic</sub>

A fast, native Wayland launcher for Sway: applications, executables and clipboard
history with fuzzy matching and frecency. Built in Rust on
[Rune](https://github.com/anomalyco/rune).

gofi keeps the keyboard-first feel of rofi — the Catppuccin Macchiato palette
and JetBrains Mono text — and adds a ranking model that puts what you use most
first without letting history override a clearly better match. The panel is
quieter than rofi: a tinted selection pill, text tabs and short spring
transitions, with motion removable in one switch.

> **Status:** pre-alpha, daily-drivable on Sway. Apps, run and clipboard modes,
> icons, launch/copy, frecency and single-instance handling work. The window
> switcher and packaging are not done.

## Features

- **Three modes:** apps (XDG desktop entries with icons), run (`$PATH`
  executables), and clipboard (`cliphist` history).
- **Fuzzy matching** with smart case and Unicode normalization. Matched
  characters are bold and underlined, as in rofi.
- **Frecency ranking** for apps and run: a decayed use count persisted in
  `$XDG_STATE_HOME/gofi/state.json`.
- **Clipboard stays in cliphist.** gofi reads `cliphist list`, decodes the chosen
  entry with `cliphist decode`, and pipes it to `wl-copy`. Previews and entry IDs
  are never written to gofi state or logs.
- **Native Wayland toplevel**, no X11. An 800×496 panel centred on the focused
  Sway output, with seven 48 px rows, 32 px icons, a tinted selection pill and
  an 8 px scrollbar.
- **Keyboard-first:** arrows and paging, Tab for modes, Enter to launch or copy,
  Escape to close, plus readline-style editing keys.
- **Single instance:** invoking gofi while it is open closes the open launcher.

## Requirements

- Linux with Sway (Wayland). X11, macOS and Windows are out of scope.
- Rust 1.99.1, pinned by `rust-toolchain.toml`.
- [`just`](https://github.com/casey/just) for the recipes.
- For clipboard mode: `cliphist` and `wl-clipboard` (`wl-copy`).

## Build and install

gofi builds against a pinned Rune revision with two local patches: a startup fix
and rofi's 2 px match underline. `references/rune` is a pristine checkout of that
revision; `vendor/rune` is the scratch checkout the build uses. `just setup-rune`
fetches the pin and applies every `docs/research/rune-*.patch`; `just build` and
`just check` run it automatically.

```sh
git clone https://github.com/geril07/gofi
cd gofi
just build
just install        # copies to ~/.local/bin/gofi; set BIN_DIR to override
```

A plain `cargo build` needs `just setup-rune` first. It is idempotent.

## Sway setup

gofi is a normal toplevel with `app_id="gofi"`. Add the window rule and whatever
keybinds you prefer:

```sway
for_window [app_id="gofi"] floating enable, border none, resize set 800 px 496 px, move position center

bindsym $mod+Return exec gofi
bindsym $mod+Shift+Return exec gofi run
bindsym $mod+Ctrl+c exec gofi clipboard
```

`move position center` follows the focused workspace, so the panel opens on the
monitor you are using. Do not use `move absolute position center`: with more than
one output it centres across the whole output layout instead.

With a bar, the panel centres in the workspace below it and sits slightly lower
than rofi's output-centred layer-shell surface. The size is fixed, so `gaps
inner` moves the centre but does not narrow the panel.

## Usage

```sh
gofi                 # apps
gofi run             # executables on $PATH
gofi clipboard       # cliphist picker
```

Headless, no window:

```sh
gofi --dump --mode apps --query fire
gofi --dump --mode run --limit 10
```

`--dump` prints ranked items as JSON and exits.

### Motion

Selection, the tab indicator and mode changes use Rune springs. Set
`GOFI_MOTION=off` (also `0`, `false`, `no`, `none`) to land every change at once;
the resting UI is identical.

### Keys

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

## How it works

- `crates/gofi-core` — app, run and clipboard providers; fuzzy ranking with
  frecency; the state store.
- `crates/gofi` — Rune UI, Sway geometry, icons, and launch/copy actions.

Clipboard history is cliphist's. gofi asks it for the list, shows the previews,
and on Enter decodes the selected entry into `wl-copy`; it keeps no copy of its
own.

## Development

```sh
just check       # fmt, clippy and the test suite
just e2e         # nested headless Sway: open, type a query, close
just e2e-launch  # nested headless Sway: launch and clipboard actions
just measure     # startup distribution in the nested compositor
```

GUI tests run in a nested headless Sway with an isolated runtime directory and
never touch a live session. See [docs/verification.md](docs/verification.md) for
the harness and the measured numbers.

## Project docs

- [Goals](docs/goals.md)
- [Architecture](docs/architecture.md)
- [Verification](docs/verification.md)
- [Status](docs/status.md)
- [Decisions](docs/decisions/)
- [Rune assessment](docs/research/rune-assessment.md) and the local
  [Rune patches](docs/research/)

## License

MIT. The embedded JetBrains Mono files are under the SIL Open Font License 1.1;
see `crates/gofi/assets/JetBrainsMono-OFL.txt`.

Rofi is the design reference for geometry, colours and key handling. Rune
supplies the Wayland window and UI runtime.
