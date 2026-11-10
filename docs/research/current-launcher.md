# Current launcher behaviour (rofi)

Snapshot of what gofi must feel like. Source: `references/sway/config`,
`references/rofi/config.rasi`, `references/rofi/1-14.rasi`,
`references/rofi/catppuccin-macchiato.rasi`, and `rofi 2.0.0` on the owner's
machine (2026-11-10).

## Sway bindings

| Binding | Command | Purpose |
|---|---|---|
| `Mod+Return` | `rofi -show drun` | applications |
| `Mod+Shift+Return` | `rofi -show run` | executables |
| `Mod+Shift+w` | `rofi -show window` | window switcher (future) |
| `Mod+Ctrl+c` | `cliphist list \| rofi -dmenu \| cliphist decode \| wl-copy` | clipboard (future) |

`$term` is `kitty`; `$mod` is `Mod4` (Super). `focus_on_window_activation smart`.
There is an existing `for_window [app_id="fzf-launcher"]` rule, a previous experiment.

## Rofi invocation

- `rofi -show drun` and `rofi -show run` are separate processes.
- `config.rasi`:
  - `modes: "drun,run,window"`
  - `font: "JetBrainsMono 16"`
  - `show-icons: true`
  - `drun-display-format: "{name}"`
  - `drun-match-fields: "name"`
  - `sort: false`
  - `matching: "fuzzy"`
  - `case-smart: true`
  - `cycle: true`
  - `scroll-method: 1`
  - `window-format: "{w}    {c:20}   {t:30}"`
  - `kb-mode-next: "Shift+Right,Tab"`
  - `kb-mode-previous: "Shift+Left,ISO_Left_Tab"`
  - `kb-cancel: "Escape,Control+g,Control+bracketleft,Control+c"`
  - `me-accept-entry: "MousePrimary"`
  - `inputchange { action: "kb-row-first" }`
  - `timeout { action: "kb-cancel"; delay: 0 }`
- `1-14.rasi` theme:
  - window: `transparency: real`, `location: center`, `anchor: center`,
    `border: 0`, `border-radius: 0`, background `@background` (mantle).
  - mainbox: `spacing: 20px`, `margin: 10px 20px`, transparent, children
    `inputbar`, `message`, `listview`.
  - inputbar: transparent, radius 8px, children `textbox-prompt-colon`, `entry`,
    `mode-switcher`.
  - prompt: padding 15px.
  - textbox-prompt-colon: disabled (search glyph is the prompt).
  - entry: padding 15px, placeholder `"Search..."`.
  - listview: `lines: 7`, `cycle: false` (the global config also sets
    `cycle: true`; verify which wins in the screenshots), `dynamic: true`,
    `scrollbar: true`, `spacing: 20px 10px`, transparent.
  - scrollbar: handle 8px, radius 8px, background alt.
  - element: padding 15px, radius 8px, spacing 15px, transparent normal;
    selected = blue background, mantle text.
  - element-icon: 32px.
  - element-text: vertical align 0.5, horizontal 0.0.
  - mode-switcher: three buttons, width 100px, padding 15px, radius 8px,
    alternate background; selected = blue background, mantle text.
  - message/textbox: alternate background, radius 8px, padding 15px.
- `4-9.rasi` is an alternate theme (not the active one; `config.rasi` imports
  `1-14`).

## Palette (Catppuccin Macchiato)

```
base #24273a   mantle #1e2030   crust #181926
surface0 #363a4f  surface1 #494d64  surface2 #5b6078
overlay0 #6e738d  overlay1 #8087a2  overlay2 #939ab7
text #cad3f5  subtext1 #b8c0e0  subtext0 #a5adcb
blue #8aadf4  lavender #b7bdf8  green #a6da95  red #ed8796
peach #f5a97f  yellow #eed49f  teal #8bd5ca  sky #91d7e3
sapphire #7dc4e4  mauve #c6a0f6  pink #f5bde6
flamingo #f0c6c6  rosewater #f4dbd6  maroon #ee99a0
```

Theme mapping: background = mantle, alternate background = base, foreground =
text, selected = blue, active = green, urgent = red.

## Behaviour to preserve

- Opens centred with the query empty and focused.
- Results are a single vertical list with icons and names.
- Typing filters immediately; the first row is selected.
- `Up`/`Down` move the selection; the list cycles (config `cycle: true`).
- `Tab`/`Shift+Tab` switch modes (if the mode switcher is shown).
- `Enter` accepts; `Escape`/`Ctrl+G`/`Ctrl+[`/`Ctrl+C` cancel.
- Mouse click on a row accepts; scroll moves the selection.
- The window closes on focus loss (rofi default; verify against the owner's
  experience).
- `run` shows executables from `$PATH`; `drun` shows `.desktop` applications.
- `drun` matches on `Name` only, not `GenericName`/`Keywords`; gofi may match more
  fields, but the displayed title stays `Name`.

## Known dissatisfaction

- `sort: false` and no frecency: frequently used apps do not reliably rise to the
  top.
- The owner's main concern is sorting/ranking, not the visual design.

## Screenshots

TODO: the owner offered screenshots of both modes. Add them under
`references/screenshots/` (gitignored) and record the measured width, height, row
height, padding and mode-switcher layout here. Until then, the values above are the
source of truth.
