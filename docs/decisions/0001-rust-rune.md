# 0001 — Rust + Rune

- **Status:** accepted
- **Date:** 2026-11-10

## Context

gofi needs a native Wayland UI, fast startup, text input, a scrollable list, icons
and a small binary. The owner wants to evaluate Rune, a native GPU UI framework in
Rust (wgpu + winit), and the project is a good excuse. Rune is pre-release, MIT
licensed, and already has the pieces a launcher needs: windows, text input, a
headless renderer/tree for tests, `set_app_id`, and layer-shell overlays.

Rune builds on this machine with Rust 1.99.1, and a real Rune window renders under
nested headless Sway (verified 2026-11-10).

## Decision

- Rust 2024, one binary named `gofi`.
- Depend on Rune from git, pinned to a `dev` revision:
  `04b38ae0a73bda03f010e97d2fff540ab91f67b0`.
- Use Rune for the window, rendering, text input, layout and headless tooling.
- Do not fork Rune for MVP. If a blocker appears, record it and evaluate a minimal
  patch or an upstream issue before forking.

## Consequences

- Rune is pre-release; its API will move. Updating the rev is a deliberate task with
  changelog review and re-verification, not a background bump.
- The build requires Rust 1.99.1 (`rust-toolchain.toml` pins it).
- wgpu device creation is part of cold start; measure it before committing to a
  one-shot process model.
- Rune's own warnings are not ours; our CI runs clippy on our crate only.

## Revisit when

- Rune cannot provide keyboard focus, text input or a usable launcher surface on
  Wayland after a reasonable spike.
- Cold start is far outside the budget and cannot be fixed with a resident process.
- The pinned rev breaks the build and upstream has no fix.
