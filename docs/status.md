# Status

**Last updated:** 2026-11-10
**Phase:** M0 — validation and infrastructure

## Done

- Reconnaissance of the current setup:
  - rofi 2.0.0, Catppuccin Macchiato, JetBrains Mono 16, 32 px icons, 7 rows.
  - Sway keybinds: `Mod+Return` drun, `Mod+Shift+Return` run, `Mod+Shift+w` window,
    `Mod+Ctrl+c` clipboard.
  - rofi config copied to `references/rofi/`; Sway config to `references/sway/`.
- Cloned Rune at `04b38ae0a73bda03f010e97d2fff540ab91f67b0` to `references/rune`.
- Cloned rofi 2.0.0 source to `references/rofi-src`.
- Installed Rust 1.99.1; Rune `cargo check -p rune` passes.
- Built `examples/hello`; rendered a real Rune window under nested headless Sway
  (wgpu + pixman, 1280×800) and captured a screenshot. Sway floated/centred it.
- Measured `hello` readiness under nested Sway: debug **280–310 ms** (5 runs);
  release **90 ms median** (9 warm runs, 240 ms first run). This is process start →
  first `on_ready`, not compositor presentation. One-shot process is viable; the
  daemon is not needed for MVP (ADR 0007).
- Launcher surface spike passed (Rune 700×480 borderless transparent window):
  - `rune::set_app_id("gofi")` → Sway sees `app_id="gofi"`.
  - `for_window [app_id="gofi"] floating enable, border none, move position center`
    + `resize set 700 480` → exactly 290,160 700×480 on a 1280×800 output.
  - Text input receives typed keys; `Escape` closes; focus loss closes when a real
    focus change occurs.
  - Harness caveat: `wtype` owns the only keyboard in nested headless Sway; when it
    exits, the seat loses keyboard capability and Rune reports focus loss. Keep a
    virtual keyboard alive during focus-loss tests. Real sessions with a physical
    keyboard are unaffected. Documented in `verification.md`.
- E2E harness: `scripts/e2e-sway.sh` starts a nested headless Sway, asserts the
  socket is private, checks `app_id`/focus, captures screenshots, types a query,
  and checks Escape closes the window. Verified with the surface spike:
  `PASS` (evidence in `target/evidence/e2e-spike/`).
- Wrote the initial docs: orchestration, goals, architecture, verification, ADRs
  0001–0007, this status, and the Rune assessment.

## In progress

Nothing. M0 spike work is next.

## Next (ordered)

1. **Scaffold the Cargo project** and implement the headless core (`--dump`) before
   the UI: desktop parser, PATH scan, ranking, frecency, state.
2. **First UI slice** on top of the core: real item list, icons, selection, launch.
   The surface spike proved the window/focus/text path; do not rebuild it blindly.
3. **Sway integration + measurements** on the real app: window rule, keybinds,
   cold start with providers, daily-use checklist.

## Blockers

None hard.

## Risks

- Cold start may be too slow for a one-shot process; daemon fallback is designed but
  unbuilt.
- IME/text input on Wayland needs a real test.
- Rune is pre-release; API churn is expected.
- Layer-shell immediate keyboard focus is not available; the ordinary-window path is
  the workaround (ADR 0003).

## Open questions for the owner

1. Screenshots of the current rofi apps/run modes (spacing, width, row height, mode
   indicator).
2. Two modes vs one combined list. Default: two modes, preserve keybinds.
3. Resident daemon acceptable if cold start misses the target?
4. Name (`gofi`) and license (MIT?).
5. Confirm window switcher and clipboard stay out of MVP.

## References

- [Goals](goals.md)
- [Orchestration](orchestration.md)
- [Architecture](architecture.md)
- [Verification](verification.md)
- [Rune assessment](research/rune-assessment.md)
- [Current launcher](research/current-launcher.md)
- [Decisions](decisions/)
