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
- The selection is a **tinted pill** (blue at 16%) with a 2 px accent rail, not a
  filled bar. Mode tabs are text labels with a 40×2 underline, not icon blocks.
  No metadata column, footer or preview pane.
- Motion is its own module, `ui/motion.rs`. Selection travels on a 0.24 s spring,
  the tab underline is a shared element on 0.22 s, and a mode swap slides the
  incoming list 10 px while the outgoing one fades (0.18 s / 0.14 s). Typing is
  instant; there is no open animation.
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

## Revisit when

The owner wants a compact/top-anchored layout, a clipboard preview pane, or
compositor blur/dim; or when motion measurably hurts startup, frame rate or
keyboard latency.
