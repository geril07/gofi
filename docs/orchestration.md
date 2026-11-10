# Orchestration

Canonical operating manual for building **gofi**. Read this at the start of every
session and after every compaction, before touching code. If a chat message and this
file disagree, ask the owner; do not silently choose.

- Product definition: [`goals.md`](goals.md)
- Current state: [`status.md`](status.md)
- Architecture sketch: [`architecture.md`](architecture.md)
- Verification: [`verification.md`](verification.md)
- Decisions: [`decisions/`](decisions/)

## 1. Roles

| Role | Who | Owns | Limits |
|---|---|---|---|
| **Orchestrator** | primary session | direction, task briefs, integration, docs, final verification, user communication | does not write large features; keeps the plan honest |
| **Worker** | subagent | one bounded implementation task in an isolated worktree | max **2 concurrent**; verifies its own work end to end |
| **Researcher** | subagent | external evidence for one blocked decision | only when a decision cannot be made from the repo/docs; returns links + recommendation |
| **Reviewer** | subagent | read-only adversarial review of an integrated diff or plan | risk-triggered; P0–P3 findings |
| **Verifier** | subagent | independent end-to-end verification through real interfaces | **exception-only**; max 2, one at a time |

**No subagent may spawn another subagent.** The orchestrator is the only dispatcher.

### Why workers self-verify

The owner's rule: a worker is not done when the code compiles. It is done when the
changed behavior was exercised through the real path and the evidence is saved. This
catches integration mistakes earlier than a separate verifier and keeps the verifier
for genuinely independent checks (milestones, focus/window behavior, release builds).

A verifier is used only when:

1. a milestone is complete and the owner wants an independent pass;
2. the change affects the real window/keyboard/launch path and worker evidence is
   inconclusive;
3. a regression is suspected in code the worker did not touch.

A verifier does **not** fix code. It reports `PASS`, `FAIL` or `INCONCLUSIVE` with
artifacts and exact reproduction steps.

## 2. Model and retry policy

All subagents use **`opencode-go/deepseek-v4.1-flash`**. No fallback to any other model.

- **Transient failure** (network, provider 5xx, timeout, empty response): retry the
  same task with exponential backoff: 30 s, 60 s, 120 s, 240 s, 480 s. Maximum five
  attempts. Resume the same subagent session when the failure is transport-level;
  start a fresh session if the context is corrupted.
- **Monthly/quota/credit exhaustion** (a limit that will not clear by retrying):
  stop immediately, do not switch models, report to the owner with the exact error.
- **Model unavailable but not a quota limit**: keep retrying within the schedule.
  If all five attempts fail, stop and report.
- Log a retry in `status.md` only when it delays a milestone or changes the plan.

The orchestrator never silently delegates to a cheaper/different model. If the owner
asks for a different model explicitly, that overrides this section for that task.

## 3. Pipeline

Every unit of work goes through the same loop. Small fixes can skip research and
review, but never skip self-verification.

### 0. Orient

Read `orchestration.md`, `goals.md`, `status.md`, and the latest ADRs. Check the
working tree and the last verification evidence. Do not start from chat memory.

### 1. Frame

Write a task brief (in `status.md` or `docs/tasks/<id>.md` if it is large):

```
### Task <id>: <title>
Goal:            one observable outcome
Scope:           files/modules the worker may touch
Out of scope:    what must not change
Acceptance:      observable criteria, including failure cases
Verification:    exact commands + what evidence proves it
Risks:           what could break, what to watch
```

A task is too big if it cannot be verified independently in one pass. Split it.

### 2. Research (only if blocked)

Use a researcher when the answer is outside the repo and a wrong choice is expensive:
library API, Wayland protocol behavior, Rune capability, algorithm comparison. Give
the researcher the question, the constraints, and the decision deadline. Require
source links and a recommendation with tradeoffs. Record the outcome in an ADR if it
changes architecture.

Do not research what `references/`, the repo docs, or a 10-minute spike can answer.

### 3. Implement

- One worker per task; at most two workers concurrently.
- Parallel workers must have **disjoint file ownership**. If two tasks need the same
  file, serialize them.
- Each worker gets a git worktree under `.worktrees/<task>` (gitignored) or a branch
  when the task is sequential.
- The worker reads this file, the task brief, and the relevant ADRs before editing.
- The worker does not edit `docs/decisions/`, `references/`, or another worker's files.
- The worker does not add dependencies without orchestrator approval (an ADR may be
  required).

### 4. Self-verify (mandatory)

The worker runs, at minimum:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

and then exercises the changed behavior through the real interface:

- pure logic → a focused test with real inputs (temp dirs/files, fake executables);
- UI structure → Rune headless `--snap` / `--tree` with a deterministic scene;
- window, focus, keyboard, launch → nested headless Sway + `wtype` + `swaymsg` +
  `grim` (see [`verification.md`](verification.md));
- performance-sensitive change → the measurement script, with numbers.

Evidence goes to `target/evidence/<task>/` (logs, screenshots, tree dumps, timings).
The worker's final report must include: summary, files changed, commands + results,
evidence paths, risks, follow-ups. No raw log dumps in the chat report.

**Three-strike rule:** if the same error survives three focused attempts, stop and
report the error, what was tried, and the smallest next experiment. Do not thrash.

### 5. Integrate

The orchestrator reviews the diff, checks file ownership, runs the full check suite on
the integrated tree, and resolves conflicts. Integration is sequential even when
implementation was parallel.

### 6. Review (risk-triggered)

Run an independent reviewer before merging when a change touches:

- ranking, scoring, frecency, or result ordering;
- persisted state format or migration;
- process spawning, `Exec` parsing, or anything that executes external commands;
- Wayland window/focus/keyboard behavior;
- a new dependency, or a Rune rev update;
- more than ~400 changed lines, or a milestone boundary.

Reviewer output is ordered **P0–P3**. P0/P1 block merge; P2/P3 become follow-ups
unless the owner says otherwise. The reviewer is read-only and does not rewrite code.

### 7. Independent verification (exception-only)

A verifier runs the real binary in the nested Sway harness (or the real CLI) against
the acceptance criteria. It saves artifacts and returns a verdict. It does not read
the worker's conclusions first; it derives the checks from the task brief.

### 8. Record

Update `status.md` (done/next/blockers), add or amend an ADR if a decision changed,
and commit with a message that says what behavior changed and how it was verified.
A milestone is not complete until `status.md` and the evidence agree.

## 4. Decision policy

Write an ADR when a choice is expensive to reverse or will be depended on by later
code:

- language, framework, renderer, windowing model;
- ranking algorithm and scoring policy;
- state format and storage;
- process model (one-shot vs resident);
- public CLI and config format;
- testing/verification infrastructure;
- dependency that owns hard behavior.

ADR format: `docs/decisions/NNNN-short-title.md` with **Status**, **Date**,
**Context**, **Decision**, **Consequences**, **Revisit when**. Keep it short. A
decision can be `proposed`; do not implement against a `proposed` ADR without the
owner's approval when it changes scope.

Reversible choices (naming, internal module split, a helper crate) do not need an ADR.

## 5. Concurrency and isolation

- Max 2 workers. Max 2 verifiers, one at a time, exception-only.
- One writer per file at a time. The task brief names the files.
- Worktrees live under `.worktrees/<task>`; remove them after merge.
- Never run two GUI processes against the same nested Sway instance.
- Never run a worker and a verifier on the same files at the same time.
- The orchestrator owns merges and the final `status.md` update.

## 6. Safety rules

- **Never touch the owner's live Sway session.** E2E always uses a nested headless
  Sway with its own `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `SWAYSOCK`, `I3SOCK`.
  Unset `SWAYSOCK`/`I3SOCK` before starting; pass `swaymsg -s <nested-socket>`
  explicitly; assert the socket path is under the temp runtime dir.
- Do not run GUI probes on the owner's display. Do not use `swaymsg` without `-s`.
- Do not execute shell strings from `.desktop` files. Parse `Exec` into argv.
- Do not commit secrets, tokens, personal paths, or session data. Keep evidence
  under `target/` (gitignored).
- Destructive commands (delete a worktree, reset a branch) require explicit owner
  confirmation when they can lose work.

## 7. Stop and escalate

Stop and ask the owner when:

- the product behavior is ambiguous and both choices are plausible;
- the correct fix is larger than the task (scope expansion);
- a hard blocker appears (Rune cannot do the required Wayland behavior, a dependency
  is unusable, the build cannot be made reproducible);
- three focused attempts fail on the same error;
- a monthly/quota limit is hit;
- a change would touch the live desktop or the owner's data.

Report: what happened, what was tried, the evidence, the options, and a recommendation.
Do not bury the decision in a wall of logs.

## 8. Definition of done

A task is done when all are true:

- acceptance criteria are met and demonstrated;
- `fmt`, `clippy -D warnings`, tests, and release build pass;
- the changed behavior was exercised through its real interface;
- evidence is saved and referenced;
- docs/ADRs/status are updated;
- no unresolved P0/P1 review finding;
- the diff is integrated and committed.

## 9. Current phase

**M1 — core model.** M0 is complete: Rune builds, a real window renders under nested
headless Sway, startup is measured (90 ms median release readiness), text input and
focus-loss close are verified, the Sway rule is exact, and
`scripts/e2e-sway.sh` is a working harness. Clipboard is now in MVP (ADR 0008).
The next work is the headless core (`--dump`): desktop parser, PATH scan, cliphist
provider, ranking, frecency and state, before the UI.
