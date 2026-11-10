# Status

**Last updated:** 2026-11-10
**Phase:** M0 complete → M1 — core model

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
- Launcher surface spike passed (Rune 700×480 borderless transparent window; the
  real target is 960×495):
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
- Owner provided screenshots (`references/screenshots/`, gitignored). Measured the
  real look: **960×495** panel, 902×48 selected row, 57 px pitch, 100×59 mode tabs
  with 21 px gap, 8 px scrollbar, matched characters underlined. The tabs are the
  mode switcher; the folder/filebrowser tab is dropped.
- **Clipboard is in MVP.** It uses the existing `cliphist` database and
  `cliphist decode | wl-copy`; no own history, no clipboard content in state/logs.
  ADR 0008. Verified the pipeline with a private DB in nested Sway:
  `store` → `list` → `decode` → `wl-copy` → `wl-paste` returned the test value.
- Real-config geometry check: ran the owner's actual Sway config (startup include
  removed) in nested Sway with the real rofi. Rofi is layer-shell, 50% width ×
  495 logical height, centred: 960×495 at 480,293 on 1920×1080; 853×495 logical
  (1279×742 physical) on 2560×1600 @ 1.5. The gofi rule
  `resize set 50 ppt 495 px` matches both exactly. Verified at both outputs.
- Wrote the initial docs: orchestration, goals, architecture, verification, ADRs
  0001–0008, this status, and the Rune assessment.

## In progress

M1 core, two workers in isolated worktrees:

- **Worker A** — providers + icons: `.worktrees/m1-providers`,
  [task 0001](tasks/0001-m1-providers.md).
- **Worker B** — ranking + frecency + state: `.worktrees/m1-ranking`,
  [task 0002](tasks/0002-m1-ranking.md).

Both use `opencode-go/deepseek-v4.1-flash`; no model fallback. The orchestrator
will integrate, wire the final `--dump`, and run the full checks.

## Next (ordered)

1. **Scaffold the Cargo project** and implement the headless core (`--dump`) before
   the UI: desktop parser, PATH scan, cliphist provider, ranking, frecency, state.
2. **First UI slice** on top of the core: 50% width × 495 height window, mode tabs, icons, match
   underlines, selection, launch/copy. The surface spike proved the
   window/focus/text path; do not rebuild it blindly.
3. **Sway integration + measurements** on the real app: window rule, keybinds,
   cold start with providers, clipboard e2e, daily-use checklist.

## Blockers

None hard.

## Risks

- Real-app cold start (desktop parse + PATH scan + cliphist) is unmeasured; the
  minimal app is 90 ms median, so the budget is likely safe.
- IME/composed text input on Wayland needs a real test.
- Clipboard previews are sensitive; never log them or persist them. The provider
  must stream decode → wl-copy.
- Rune is pre-release; API churn is expected.
- Layer-shell immediate keyboard focus is not available; the ordinary-window path is
  the workaround (ADR 0003).

## Open questions for the owner

1. Name (`gofi`) and license (MIT?).
2. Confirm the window switcher stays out of MVP.

Resolved: three tabs (Apps / Run / Clipboard), `cliphist` as the clipboard source,
and the 50% × 495 geometry verified against the real config.

## References

- [Goals](goals.md)
- [Orchestration](orchestration.md)
- [Architecture](architecture.md)
- [Verification](verification.md)
- [Rune assessment](research/rune-assessment.md)
- [Current launcher](research/current-launcher.md)
- [Decisions](decisions/)
