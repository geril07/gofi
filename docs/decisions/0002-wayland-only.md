# 0002 — Wayland only, Sway first

- **Status:** accepted
- **Date:** 2026-11-10

## Context

The owner runs Sway (`swayfx 0.6`, Sway 1.12) on Wayland. rofi 2.0 is the current
launcher. The request is explicitly native Wayland: desktop/application scans,
executable scans, no X11 fallback. Supporting X11, macOS and Windows would multiply
the surface area and slow the MVP.

## Decision

- Native Wayland only. No X11 backend, no XWayland fallback for the launcher.
- Sway is the first-class compositor. Use Sway IPC (`swaymsg`) and `wtype` in tests.
- Do not depend on compositor-specific protocols beyond what Sway/wlroots provides.
  The MVP uses an ordinary xdg-toplevel window, not layer-shell (see ADR 0003).
- Keep compositor integration behind a thin boundary so a future layer-shell or
  other-compositor backend does not leak into providers/ranking.

## Consequences

- GNOME/Mutter is not supported in MVP (no layer-shell; window rules differ).
- Sway window rules are part of the integration; document them in the install
  instructions.
- E2E runs on nested headless Sway, which is also the development harness.

## Revisit when

- The owner uses another compositor.
- Rune's layer-shell path becomes suitable for an immediate keyboard launcher and
  the toplevel workaround causes real problems.
