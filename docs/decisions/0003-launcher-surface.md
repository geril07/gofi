# 0003 — Launcher surface: ordinary borderless window for MVP

- **Status:** accepted (revisit after M0)
- **Date:** 2026-11-10

## Context

A launcher wants a panel-like surface: centred, borderless, no taskbar entry, takes
keyboard immediately, closes on focus loss. Rune has two relevant paths:

1. **Layer-shell overlay** (`App::overlay`, `platform/layer_shell.rs`). On Wayland it
   creates a `zwlr_layer_shell_v1` surface on the overlay layer. The code supports
   pointer and keyboard dispatch, but creation sets
   `KeyboardInteractivity::None`; `request_keys()` switches to `OnDemand` (requires
   layer-shell v4). `OnDemand` means the compositor gives keyboard focus when the
   user clicks the surface, not immediately on show. Rune's README also states that
   interactive overlays are unavailable on Wayland (pointer events on the private
   queue are not wired into the app runtime). That is a poor fit for a
   type-immediately launcher.
2. **Ordinary window** (`WindowOptions::launcher_panel()` on macOS; an ordinary
   initially hidden window elsewhere). On Wayland this is a normal xdg-toplevel. It
   takes keyboard focus when opened by a Sway keybind, supports text input and
   pointer events, and Sway can float/centre it with a window rule. `rune::set_app_id`
   sets the Wayland `app_id` so the rule matches.

## Decision

Use an ordinary borderless, transparent xdg-toplevel window for the MVP:

- `rune::set_app_id("gofi")` before `run`.
- `WindowOptions::launcher_panel()` (hidden, borderless, transparent, non-resizable)
  or an equivalent explicit `WindowOptions` on Linux.
- Sway rule:
  `for_window [app_id="gofi"] floating enable, border none, move position center`
  plus `resize set 960 495` (the screenshot target; verify on the 1.5× output).
- Close on focus loss (`App::on_focus`), reopen on the next keybind.
- Do not use `App::overlay` for the launcher until Rune can grant immediate keyboard
  focus to a layer-shell surface.

## Verification (2026-11-10)

A throwaway Rune app with `set_app_id("gofi")` and a 700×480 borderless transparent
window (the spike size; the real target is 960×495, measured from the owner's
screenshots) was run under nested headless Sway:

- Sway saw `app_id="gofi"`, `focused=true`, rect `290,160 700×480` (centred).
- Typing reached a focused `text_input`; `Escape` closed the window.
- A real focus change (another window taking focus) closed the window.
- `on_focus(false)` also fires when the nested seat loses keyboard capability because
  `wtype` exits; that is a harness artifact, not a real focus loss. Keep a virtual
  keyboard alive in tests (see `docs/verification.md`).

## Consequences

- The window is a toplevel, so the compositor may animate it and it appears in the
  window tree. Sway rules are required; they are part of the install docs.
- No exclusive keyboard grab; focus is normal toplevel focus. That is acceptable for
  a personal launcher.
- A resident process that recreates the window per toggle gets fresh focus on
  Wayland without activation-token work (see ADR 0007).
- If the window rule is fragile across Sway versions, the fallback is a tiny wrapper
  that issues `swaymsg` commands after launch, or a Rune layer-shell change.

## Revisit when

- Rune supports `KeyboardInteractivity::Exclusive` or immediate focus for layer-shell
  overlays, and the toplevel causes visible problems.
- The owner moves to a compositor where toplevel placement rules are unavailable.
