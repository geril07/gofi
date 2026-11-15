# 0010 — Quiet skin with separable motion

- **Status:** accepted
- **Date:** 2026-11-14

## Context

The first skin matched rofi exactly: 960×495, filled blue selection, icon tabs,
instant changes. The owner asked for a different visual direction informed by
opencode's flat command palette and Noctalia's spatial motion, with motion
removable in one step. Startup must stay level with rofi's ~90 ms first paint,
and a normal toplevel cannot dim the desktop or request compositor blur.

Rune already provides springs, transitions, shared-element flights, enter delays
and scroll glides. Its first paint lands immediately, so an open animation would
delay the content the launcher exists to show.

## Decision

- The panel is a fixed **800×496**, radius 12, 1 px hairline; seven 48 px rows,
  32 px icons and an 8 px scrollbar. Sway opens it with
  `resize set 800 px 496 px`; gofi no longer queries the output width.
- The selection is an **accent-filled pill** with panel-coloured ink, as rofi
  had it (a 16–34% tint was tried first; see the second amendment). Mode tabs are
  shrink-wrapped text labels with an 18 px gap and an underline as wide as the
  label; the underline is out of flow so selecting a tab never moves its text.
  No metadata column, footer or preview pane.
- Motion is its own module, `ui/motion.rs`. The selection lands at once, with
  the row ink it tints (amended 2026-11-15); the tab underline is a shared
  element on 0.16 s with 0.2 bounce, and a mode swap slides the incoming list
  10 px while the outgoing one fades (0.14 s / 0.11 s). Typing is instant; there
  is no open animation.
- `GOFI_MOTION=off` (also `0`, `false`, `no`, `none`) returns `Spring::NONE` /
  `Transition::none()` and marks the root `instant_changes`. The resting view is
  pixel-identical with motion on or off.

## Consequences

- The launcher no longer mirrors rofi's geometry or selection treatment; the
  README, Sway rule, e2e assertions and screenshots carry the new panel.
- Startup no longer pays a `swaymsg` process. A full-window rounded clip was
  measured at ~10–15 ms on the first frame and is not used; the rounded fill and
  border are enough because children are inset.
- Motion is additive: the skin compiles and looks correct with `GOFI_MOTION=off`.
  Deleting the module and its call sites leaves a static UI.
- The panel is still a normal toplevel, so no dim or compositor blur. A future
  layer-shell or resident variant would revisit that separately.

## Amendment (2026-11-15): selection glide without bounce

Owner review: the 0.17 s / 0.25 bounce pill read as bouncing rather than
arriving, and the earlier 0.24 s / 0 critically damped version had felt linear
and slow. The bounce was never the requested behavior; it was an agent's attempt
to make the first spring read as motion.

The selection now lands a one-row move at once. That is the arrow-key case, and
any glide there trails the highlight under key repeat. Longer moves glide on a
critically damped spring whose duration grows with the square root of the
distance, the same shape Rune's `PaneMotion::scale_with_width` uses for panes:
0.14 s at two slots to 0.24 s across the seven-row window. A wrap from the last
row to the first lands at once, because the list's window jumps with it. Typing
and mode switches still land at once.

opencode's desktop app, the visual reference for the quiet skin, has no bounce in
its motion tokens and no traveling highlight in its row lists; its one sliding
indicator (the diff-view segmented control) is 0.18 s / 0 with no overshoot. This
amendment matches that model while keeping gofi's positional pill.

## Amendment (2026-11-15): Catppuccin Mocha, accent-filled selection

The palette was carried over from the rofi theme in M0 and had never been
chosen for the new panel; owner review found the colors wrong. It is now
**Catppuccin Mocha**, the flavour Noctalia's `Catppuccin Lavender` scheme
generates, so the launcher shares a surface with the shell's panels: `base
#1e1e2e` for the panel (Noctalia's `mSurface`), `surface0 #313244` for the
scrollbar track (`mSurfaceVariant`), `text #cdd6f4`, `blue #89b4fa` and
`overlay1 #7f849c` for muted text and the hairline. The terminal and the waybar
still run Macchiato; the two flavours differ by a few percent per channel.

The selection took three attempts, and the history matters because two of them
looked plausible on paper:

- **16% tint** (the original decision). Rune composites alpha in sRGB, not in
  the linear space the palette is stored in; the rendered pill was `#2f374f`,
  **1.36:1** against the panel. A haze, not a highlight.
- **34% tint**. `#425173`, 2.07:1, text 5.47:1 — visible, but that colour is not
  a token anywhere. Catppuccin's accent is a light pastel and its base is dark,
  so blending them always desaturates to slate; the flavour has no dark
  saturated surface to use instead.
- **Accent fill** (now). The pill is the accent at full strength with panel
  ink, as rofi had it: 7.79:1 (rofi's Macchiato pair was 7.18:1). Owner review
  named the selected row's background
  as the only remaining wrong colour, and asked for the accent itself there.

The neutral alternative — `surface2 #585b70` for the selection, blue kept for
the tab underline and scrollbar — is a one-line change and would be the quiet
choice, but it makes the selection a grey bar, so it is not the default.

Any future tint must be measured from a rendered frame, not computed in linear
space: `verification.md` records the pixel samples.

## Amendment (2026-11-15): the selection does not travel

Owner review of the accent fill: the selected row's text is dark, and it must
switch colour in the same frame as the fill, with no transition. Rune springs
colours on their own track, so the ink was crossfading behind the highlight.
That is fixed with `tx.color_motion(Spring::NONE)`.

With the ink instant, the earlier distance-scaled travel no longer works: while
the pill glides across rows it covers neither the row it left nor the row it is
heading for, and the affected titles are unreadable for the length of the glide
— the old row paints `#cdd6f4` on `#89b4fa` (1.46:1) and the new one paints
`#1e1e2e` on the `#1e1e2e` panel (1.0:1). A fill that travels and an ink that
cuts are only compatible if the ink followed the fill's edge, per row.

So the selection lands at once on every move, like rofi's, and `motion::select`
is gone. The tab underline flight and the mode swap remain; the colour of the
rows is never animated. If the travel is wanted back, the fill has to go back to
a tint and the ink back to `#cdd6f4` — the two choices are one decision.

## Revisit when

The owner wants a compact/top-anchored layout, a clipboard preview pane, or
compositor blur/dim; or when motion measurably hurts startup, frame rate or
keyboard latency.
