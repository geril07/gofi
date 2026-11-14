# 0004 — Ranking: nucleo + frecency

- **Status:** accepted (constants to tune)
- **Date:** 2026-11-10

## Context

The owner's main complaint about rofi is sorting. The launcher needs a general-purpose
fuzzy matcher with good scores, plus a frequency/recency signal. Candidates:

- **nucleo-matcher** (Helix): actively used, fast, smart case, Unicode normalization,
  well-tuned bonuses for prefix/word-boundary/camel-case, multi-needle patterns.
- **frizbee** (fzf algorithm port): fast and good, but a newer API and another
  scoring model to learn.
- **fuzzy-matcher** (skim): proven, but less actively maintained.
- Hand-rolled Smith-Waterman: more control, more ways to get the scoring wrong.

The matcher is not the product differentiator; the blend with frecency is. Use a
proven matcher and spend the effort on ordering tests.

## Decision

- Use `nucleo-matcher` with `CaseMatching::Smart` and `Normalization::Smart`.
- Parse the pattern once per query; reuse the matcher and scratch buffers.
- Composite score:
  - `top = max(match_score)` over the filtered set;
  - `frecency_norm = score / (score + 10.0)` (saturating);
  - `composite = match_score + 0.15 * top * frecency_norm`;
  - sort by composite desc, then first matched character asc, then title asc.
- Empty query: sort by `frecency_norm` desc, then title asc.
- Frecency: exponential decay, half-life 14 days, updated on launch:
  `score = score * 0.5^(elapsed/HL) + 1.0`.
- Pin the intended ordering with a fixed corpus test, not exact score numbers.

The 0.15 factor means frecency can break ties and promote a slightly worse match, but
cannot overcome a clearly better one. If daily use says otherwise, tune the factor and
update this ADR.

### Start-position tiebreak (2026-11-14)

Equal composite scores used to fall straight to the title, so a match starting at
character 8 could beat one starting at 0 on its name alone: `te` ranked "Qt V4L2
test Utility" above "Telegram". The second key is the first matched character's
index in the title, so the item whose match is nearer the start wins a tie.

It only applies when composites are exactly equal, so a clearly better match and
the frecency blend are unaffected. This is the common convention: skim's default
tiebreak is `score,begin,end`, GNOME's app search orders by `token_pos`, and fzf
offers the same idea as `--tiebreak=begin`. nucleo's `prefer_prefix` would also
reward position, but it changes scores and its documentation recommends it for
autocompletion rather than pickers.

A keyword-only match has no visible position and sorts after every match that has
one.

Clipboard is still unsorted by design (ADR 0008); giving it match ordering is
tracked in [#6](https://github.com/geril07/gofi/issues/6).

## Consequences

- One new dependency (`nucleo-matcher`); small and pure Rust.
- Ranking behavior is testable without a UI.
- Score ties now resolve by match position before title; the fixed corpus test that
  relied on title order for equal scores was updated.
- The frecency key is per mode (`apps:<desktop-id>`, `run:exec:<name>`), so the same
  name in apps and run does not share history. Clipboard has no frecency and keeps
  cliphist's recency order (ADR 0008).
- Scores are not stable across nucleo versions; update tests by ordering, not numbers.

## Revisit when

- Nucleo's scoring does not match the owner's intuition after a week of use.
- A future provider needs path-aware matching (nucleo supports match paths, but the
  configuration may need to change).
- The frecency blend causes frequent-but-bad matches to outrank exact matches.
