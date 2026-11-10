# 0006 — Verification: behavior tests + nested Sway

- **Status:** accepted
- **Date:** 2026-11-10

## Context

The owner's rule: test behavior, not implementation; workers verify their own work;
UI should not be tested only through unit tests. A launcher's risk is integration:
wrong argv, wrong PATH entry, a window that does not take focus, a query that ranks
badly, a state file that corrupts.

Rune provides headless rendering, element-tree dumps and scripted input. Sway can run
headless (`WLR_BACKENDS=headless`) with its own `XDG_RUNTIME_DIR`, and `wtype` can type
into the focused client. This gives a real Wayland path without touching the owner's
desktop.

## Decision

- Unit tests for pure logic: parsing, ranking, frecency, argv, state.
- Integration tests with real temp directories and fake executables; no mocks of std.
- Headless Rune (`--snap`, `--tree`, scenes) for UI structure, not pixel goldens
  while the design is moving.
- Real e2e in a nested headless Sway: window/app_id/focus, typing, Enter launch,
  Escape, focus-loss close. Save tree JSON, screenshots and logs as evidence.
- Workers self-verify; verifiers are exception-only (milestones, risky window/launch
  changes, inconclusive evidence).
- Performance is measured and reported, not asserted in unit tests.

## Consequences

- E2E infrastructure is a first-class deliverable (M0), not an afterthought.
- Tests are slower than pure unit tests but catch the failures that matter.
- The nested-Sway harness must isolate `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`,
  `SWAYSOCK` and `I3SOCK`; a test must never touch the live session.
- No pixel-perfect assertions until the look is frozen; screenshots are for review.

## Revisit when

- The UI stabilizes enough for golden snapshots.
- A compositor-independent e2e path becomes necessary.
- Test runtime becomes a bottleneck; move slow e2e to a separate `just e2e` target.
