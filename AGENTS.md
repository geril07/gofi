# gofi

A fast, native Wayland launcher for Sway: application, executable and clipboard
search with fuzzy matching and frecency, built in Rust on
[Rune](https://github.com/anomalyco/rune). Working title; the product definition is
in `docs/goals.md`.

## Read first, and again after every compaction

1. `docs/orchestration.md` — how we work, who does what, when we stop.
2. `docs/goals.md` — scope, success criteria, milestones.
3. `docs/status.md` — what is done, what is next, open questions.
4. The ADRs in `docs/decisions/` — decisions that are expensive to reverse.

## Hard rules

- **Subagents use DeepSeek only** (`opencode-go/deepseek-v4.1-flash`). Never fall back
  to another model. Retry transient failures with exponential backoff; stop and report
  on a monthly/quota limit.
- **At most 2 workers at once.** Verifiers are exception-only (see orchestration).
  No subagent may spawn another subagent.
- **Workers verify their own work end to end.** Tests cover observable behavior, not
  implementation details.
- **Never touch the user's live Sway session.** All GUI/e2e work runs in a nested
  headless Sway with isolated `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `SWAYSOCK`, `I3SOCK`.
- **Clipboard content is sensitive.** Never log previews or decoded content, and
  never write clipboard entries or IDs to gofi state. `cliphist` owns that history.
- **`references/` is read-only.** It contains clones of Rune, rofi and the user's
  current configs. Never edit it.
- **Keep `docs/status.md` current.** Record architecture decisions as ADRs, not chat
  memory.

## Commands

- `scripts/e2e-sway.sh --binary <bin> --query <text>` — nested headless Sway
  smoke test with isolated runtime dir and evidence capture.
- The Rust project is scaffolded in M1. Planned `just` recipes are in
  `docs/verification.md`.
