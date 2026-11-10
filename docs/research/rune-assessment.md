# Rune assessment

- **Date:** 2026-11-10
- **Repo:** <https://github.com/anomalyco/rune>
- **Rev:** `04b38ae0a73bda03f010e97d2fff540ab91f67b0` (`dev`, 2026-11-10)
- **License:** MIT
- **What it is:** a native GPU-rendered Rust UI framework (wgpu + winit) with its
  own layout, text, animation, overlays and headless tooling. Pre-release; apps pin
  `dev` revisions.

## Verified on this machine

| Check | Result |
|---|---|
| Toolchain | Rust 1.99.1 installed via rustup; Rune pins it in `rust-toolchain.toml` |
| `cargo check -p rune` | Passed in ~50 s (3 upstream dead-code warnings) |
| `cargo build -p hello` | Passed in ~52 s |
| Rune window on Wayland | `examples/hello` rendered under nested headless Sway (1280×800), wgpu + pixman, screenshot captured |
| Sway rule | `for_window [title="Hello"] floating enable, move position center` centred the window |
| Debug cold start | `ready_ms` 280–310 ms over 5 runs (process start → first `on_ready`; excludes presentation) |
| Release cold start | `ready_ms` 240 ms first run, **90 ms median** over 9 warm runs (same caveat) |
| Launcher surface | 700×480 spike (real target 50% width × 495 height) borderless/transparent window, `app_id="gofi"`, Sway float/centre rule, text input, Escape, focus-loss close — all verified |
| Text input | Typed keys reach a focused `text_input`; the first key can be lost while focus settles, so tests delay before typing |
| Focus events | `on_focus(false)` also fires when the nested seat loses keyboard capability (`wtype` exits); keep a virtual keyboard alive in tests. Real physical-keyboard sessions are unaffected |

The nested Sway recipe is in [`../verification.md`](../verification.md). A first
attempt accidentally queried the owner's live Sway because `SWAYSOCK` was inherited;
the harness now unsets it and passes `swaymsg -s <nested-socket>` explicitly.

## Relevant capabilities

- **Windows:** ordinary winit windows with transparent/borderless options,
  `WindowOptions::launcher_panel()` (hidden, retained, non-resizable), `set_app_id`
  for Wayland `app_id` / X11 `WM_CLASS`.
- **Text:** `text_input` with editing, selection, IME hooks, submit and input-key
  callbacks.
- **Headless:** `--snap out.png`, `--tree out.json`, `--scene`, `--script`,
  `--at`, `--size`, `--scale`; deterministic virtual clock.
- **Overlays:** `App::overlay` / `OverlaySpec` with edge placement, input policy,
  `KeyPolicy`, layer-shell on Wayland, XWayland fallback on GNOME.
- **Accessibility:** AccessKit; the same tree is exposed to agents and screen readers.
- **Performance:** `RUNE_READY_DIR` readiness JSON, prewarm GPU, hidden-window reuse,
  idle draw suppression.

## Wayland surface findings

### Layer-shell overlay path

Source: `crates/rune/src/platform/layer_shell.rs`, `crates/rune/src/overlay/options.rs`,
`crates/rune/README.md` ("Edge overlays").

- Creates a `zwlr_layer_shell_v1` surface on the overlay layer.
- At creation: `set_keyboard_interactivity(KeyboardInteractivity::None)`.
- `request_keys()` switches to `OnDemand` (requires layer-shell v4). OnDemand means
  the compositor grants keyboard focus when the user clicks, not immediately.
- `OverlayOptions.keys = KeyPolicy::OnDemand` is the documented way to permit keys.
- The README says interactive overlays are unavailable on Wayland (pointer events on
  the private queue are not wired into the app runtime), and the overlay "never
  keyboard-focused, never activating".
- Consequence: not suitable for a type-immediately launcher without a Rune change
  (e.g. `KeyboardInteractivity::Exclusive` on show) or a custom surface.

### Ordinary window path

- A normal xdg-toplevel opened by a Sway keybind gets keyboard focus normally.
- Sway can float/centre it with `for_window [app_id="gofi"]`.
- `rune::set_app_id("gofi")` is applied to ordinary windows in
  `crates/rune/src/window/create.rs` (Wayland `with_name`, X11 `WM_CLASS`).
- This is the MVP path (ADR 0003).

## Risks

1. **Cold start.** Rune creates a wgpu device and pipeline. Release `hello` measured
   90 ms median readiness (240 ms first run); the real app adds desktop parsing,
   ranking and icons, so measure again. If it misses the target, the resident daemon
   fallback is designed (ADR 0007).
2. **IME on Wayland.** Rune's text stack is its own; IME arrives through the platform
   shell (winit at this stage). Basic typing works; composed input needs a real test.
   For the owner's English queries this is low risk, but it must be verified before
   calling the launcher done.
3. **Pre-release API churn.** Pin the rev; update deliberately; keep Rune-specific
   code behind a thin app module so a framework change is localized.
4. **Window semantics.** A toplevel may animate, appear in the tree, or be affected by
   Swayfx corner radius/blur. Test with the real Sway config and add a rule.
5. **Focus loss.** `App::on_focus(false)` closes the launcher on a real focus change
   (verified). In nested headless Sway, `wtype` owns the only keyboard; when it exits,
   the seat loses keyboard capability and Rune reports focus loss even though the
   window is still focused. Keep a virtual keyboard alive in focus-loss tests. This is
   a harness artifact, not a real-session bug.
6. **Startup measurement noise.** Use `RUNE_READY_DIR` and report medians/p95, not a
   single run; state what the number includes.

## Recommendation

Proceed with Rune and the ordinary-window path. The framework is capable enough for
the MVP: startup is fast, the window takes focus, text input works, and the Sway rule
is exact.

Verified with the real app (2026-11-11): the window renders the measured rofi layout
under nested Sway, launch and clipboard actions pass e2e, and process start → first
`on_ready` is apps median 300 ms / p95 350 ms (20 runs, nested pixman), within the
400 ms target. Embedding JetBrains Mono removed a ~100 ms fontconfig cost. The
remaining unknown is IME/composed input; it needs a real session test.
