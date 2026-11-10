# Task 0002 — M1 ranking and state

- **Status:** in progress
- **Owner:** worker (DeepSeek)
- **Worktree:** `.worktrees/m1-ranking`

## Goal

Implement fuzzy ranking, frecency and persistent state in `gofi-core`, with
behavior tests. The CLI `gofi --dump` must return correctly ordered items.

## Scope

Only these files:

- `crates/gofi-core/src/ranking/mod.rs`
- `crates/gofi-core/src/ranking/frecency.rs`
- `crates/gofi-core/src/store.rs`
- `crates/gofi-core/tests/ranking_state.rs` (new)

Do not change `model.rs`, `xdg.rs`, `providers/`, `icons.rs`, `crates/gofi/`,
`Cargo.toml`, `docs/`, or `references/`. If a dependency is genuinely required,
stop and report instead of adding it.

## Acceptance

### Ranking

- Use `nucleo-matcher` with `CaseMatching::Smart` and `Normalization::Smart`.
- Parse the pattern once per query; reuse the matcher/scratch buffers.
- Filter out non-matches.
- Composite score: `match_score + 0.15 * top_match_score * frecency_norm`, where
  `frecency_norm = score / (score + 10.0)`.
- Sort by composite desc, then title asc.
- Empty query for Apps/Run: sort by frecency desc, then title asc.
- Clipboard: filter by fuzzy match but preserve the provider's original order; do
  not use frecency.
- `Ranked.positions` contains character indices in `Item::title` for underline
  highlighting. Use the matcher's indices API, not a reimplementation.
- Tests with a fixed corpus: exact/prefix/word-boundary beats scattered fuzzy;
  smart case; Unicode; frecency promotes a tie but not a clearly better match;
  empty query; clipboard order preserved.

### Frecency

- `Entry { score, last_used }`; decay half-life 14 days.
- `score(key, now)` applies decay without writing.
- `record(key, now)` decays then adds 1.0 and sets `last_used`.
- Tests with a fake clock, including zero/old entries.

### State

- JSON v1 at `Env::state_file()`:
  `{ "version": 1, "modes": { "apps": { key: {score,last_used} }, "run": { ... } } }`.
- `State::load(env) -> Self`: missing → empty; corrupt/unknown version → empty +
  a warning (do not panic). Keep this signature; add a separate `load_with_warnings`
  only if needed.
- `State::save(env)`: create parent, advisory lock, write temp, fsync, rename.
- Only Apps/Run are persisted; Clipboard is never written.
- Cap each mode at 2,000 entries; evict the lowest score.
- Tests with real temp dirs: round-trip, missing, corrupt, unknown version,
  atomic replacement, lock behavior, no clipboard entries.

## Verification

Run from `.worktrees/m1-ranking`:

```sh
cargo fmt --check
cargo clippy -p gofi-core --all-targets -- -D warnings
cargo test -p gofi-core
```

Save the full output to `target/evidence/m1-ranking/check.log`. Test observable
behavior (ordering, scores after known elapsed time, file contents), not private
fields.

## Report

Return: summary, files changed, commands + results, evidence path, risks,
follow-ups. Do not edit docs or decisions. Do not spawn subagents. Do not change
the model.
