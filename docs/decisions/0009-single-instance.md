# 0009 — Single instance and toggle-close

- **Status:** accepted
- **Date:** 2026-11-11

## Context

The accepted default is that pressing the launcher keybind while gofi is open
closes it, rather than opening a second window. The launcher is a one-shot process
(ADR 0007): it starts, shows a window, and exits after an action, Escape or focus
loss. There is no daemon to receive a toggle request.

The runtime dir already exists on Wayland (`$XDG_RUNTIME_DIR`), and the process
holds a window for at most a few seconds.

## Decision

Use an advisory lock file, `$XDG_RUNTIME_DIR/gofi.lock`, held with `flock` for the
process lifetime.

- The first instance takes an exclusive non-blocking lock, truncates the file and
  writes its PID.
- A second instance fails to lock, reads the PID, and checks
  `/proc/<pid>/comm == "gofi"`. If it matches, it sends `SIGTERM` and exits 0.
  A stale PID reused by another program is not signalled.
- The kernel releases the lock when the holder exits or dies, so a stale PID does
  not block the next launch.
- `--dump` does not take the lock and never signals an open launcher.
- If `$XDG_RUNTIME_DIR` is missing or the file cannot be locked, gofi still runs;
  single-instance is a convenience, not a precondition.

## Consequences

- The second keybind press closes the open window within a few milliseconds.
- There is no IPC protocol, no socket, and no state kept between launches.
- Two instances racing to start can both reach the lock; one wins and the other
  kills it, which is the same visible result.
- If a future daemon is adopted, it replaces the lock with its toggle command; the
  CLI contract (`gofi`, `gofi run`, `gofi clipboard`) stays.
