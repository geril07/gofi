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

## Fork tradeoffs (2026-11-11)

Two framework gaps are real; the third is a perf question, not a bug.

1. **Layer-shell as the launcher window.** Rune's layer-shell is overlay-only:
   the surface is created with `KeyboardInteractivity::None`; `request_keys()`
   moves it to `OnDemand`, which focuses on click rather than on show, and the
   overlay path has no text-input/IME protocol. The README calls interactive
   Wayland overlays unsupported. A launcher surface would need immediate focus
   (`Exclusive` or an activation token) plus text-input-v3 and the app runtime,
   not the HUD runtime. This is the one functional fork/upstream candidate: it
   removes the Sway rule, places the panel at rofi's exact y, and works on other
   wlroots compositors.
2. **Underline thickness.** `Span::underline` draws `scale.max(1.0)` px and has
   no width option; rofi/Pango derives ~2 px from the font. A small upstream
   change (font metrics or a span width) closes it.
3. **Startup.** Rune already prewarms the GPU on a thread and supports
   `keep_running` plus hidden-window reuse. The 126 ms first frame is ~4 ms
   connect, ~64 ms init and window creation (including ~20 ms of app data),
   ~51 ms wgpu device, swapchain, pipelines and first draw, ~8 ms commit. A fork
   could add a wgpu pipeline cache or trim init. One-shot parity with rofi does
   not need a CPU/SHM renderer: rofi's first painted frame is ~89 ms, and the
   patched build is already at 80/90 ms. The app-level lever for going below
   that floor is the resident daemon (ADR 0007).

Border widths are not a limitation: `El::border(f32, color)` accepts any width.
Text, bold spans, icons/SVG, layout, input, image caching and the headless
runtime have all been sufficient.

## Startup patch (2026-11-13)

The real launcher paid for eager work Rune does on every launch. Measured under
nested headless Sway with `scripts/measure-startup.sh --runs 20`:

| build | median | p95 |
|---|---:|---:|
| pinned Rune (`ready_ms`) | 140 ms | 170 ms |
| patched Rune (`ready_ms`) | **80 ms** | **90 ms** |
| rofi 2.0, first painted buffer | ~89 ms | — |
| rofi 2.0, first empty commit | ~58 ms | — |

The 58 ms often quoted for rofi is spawn → its first `wl_surface.commit()`,
which is empty. The first commit carrying pixels is ~89 ms (20 alternating runs
against gofi).

What dominated the launch, and what the patch changes:

- **CJK fallbacks and `fc-list`**: `TextSystem::new` eagerly loaded up to 16
  installed CJK files and ran `fc-list` (40 ms here) for symbol families. Lazy
  per-script loading and a font-directory-first lookup took text setup from 81 ms
  to 3 ms. CJK and emoji were verified with synthetic `.desktop` entries.
- **Image workers**: `ImageCache::new` spawned four decoder threads (~14 ms).
  They now start with the first image request.
- **Reduced motion**: reading the Linux preference loads GIO and its schemas
  (~10 ms) before the first frame. It is now read after the first frame and cached.
- **Adapter pick**: `enumerate_adapters` measured 10–35 ms under nested Sway;
  `request_adapter` measured 1.6 ms. The pick now tries the adapter first and
  enumerates only when it is not the compositor's GPU.

The patch is 9 files, 138 insertions, 35 deletions. The repo carries it as a
build step:

```sh
just setup-rune   # clone references/rune into vendor/rune, apply the patch
just check-rune   # verify the pin and the applied patch
```

`scripts/setup-rune.sh` creates the gitignored `vendor/rune` from the pristine
`references/rune`; `crates/gofi/Cargo.toml` points there. Upstreaming is
deferred.

Risks:

- A first CJK or symbol/emoji string now pays the fallback load (~10–20 ms) on
  that frame. A launcher with a Latin-only query never pays it.
- `fc-list` is skipped when the standard directories have a matching file. A
  system whose preferred font lives only in a fontconfig path outside those
  directories can pick a lower-priority installed font; NixOS store paths still
  work because the directory walk finds nothing and falls back to `fc-list`.
- The reduced-motion preference is cached for the process, so a change while gofi
  runs is not followed until the next launch.
- The patch is carried locally by `scripts/setup-rune.sh` and is not upstreamed.
