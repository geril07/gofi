# 0005 — State: one JSON file, atomic writes

- **Status:** accepted
- **Date:** 2026-11-10

## Context

Frecency needs to survive restarts. The data is tiny: a few hundred items per mode,
each with a score and a timestamp. A database is unnecessary. A launcher may be
started twice concurrently (two keypresses, a test and a real run), so writes must not
corrupt the file.

## Decision

- Store state at `$XDG_STATE_HOME/gofi/state.json` (fallback
  `~/.local/state/gofi/state.json`).
- Schema:
  ```json
  {
    "version": 1,
    "modes": {
      "apps": { "<desktop-file-id>": { "score": 3.2, "last_used": 1760000000 } },
      "run":  { "exec:<name>":      { "score": 1.0, "last_used": 1760000000 } }
    }
  }
  ```
- Load: missing → empty; corrupt/unknown version → empty + warning, never a crash.
- Save: advisory lock, write temp file in the same directory, `fsync`, rename.
- Cap entries per mode (2,000) and evict the lowest score.
- Use `serde` + `serde_json` (already in Rune's tree) and `fs2` for the lock, unless
  a smaller lock implementation is already available.

## Consequences

- The file is human-readable and easy to inspect/reset.
- Last-writer-wins is avoided by the lock, but a crashed process can leave a stale
  lock; use `flock`, which the kernel releases on exit.
- No queryable history; if a future provider needs it, add a separate store rather
  than complicating this one.
- No migration machinery yet; version 1 is the only format. Add migrations when the
  schema changes.

## Revisit when

- State grows beyond a few thousand entries or writes become a measured cost.
- Multiple long-lived processes need concurrent read/write beyond a single lock.
- A future provider needs structured history (e.g. clipboard entries).
