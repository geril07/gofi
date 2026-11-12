# 0007 — Process model: one-shot first, resident daemon if needed

- **Status:** accepted — one-shot for MVP (measured 2026-11-10)
- **Date:** 2026-11-10

## Context

A launcher is judged by how fast it appears. rofi is a one-shot process and feels
instant because it is small and CPU-rendered. Rune creates a wgpu device, a window and
a renderer; Rune's own docs report about 300 ms to first content on Apple silicon
after optimization. On this Linux machine, release `hello` measured **240 ms** on
the first run and **90 ms median** over nine warm runs (`ready_ms`, process start →
first `on_ready`, excluding compositor presentation).

A resident process can keep the GPU device, parsed providers and frecency in memory.
On Wayland, hiding a window and showing it again may not grant keyboard focus
without an activation token; recreating the window per toggle gives a fresh surface
and normal focus. That costs a window + surface creation, but not a GPU device.

## Decision

1. Build the one-shot path: `gofi` runs, opens the window, launches, exits.
2. Keep it for MVP. The measured release readiness (90 ms median, 240 ms first run)
   is well inside the ≤ 400 ms target for the minimal app; the real app adds desktop
   parsing and ranking, which must be measured again.
3. If the real app misses the target, add a resident daemon:
   - `gofi toggle` connects to `$XDG_RUNTIME_DIR/gofi.sock`; if no daemon, spawn
     `gofi daemon --show` detached.
   - The daemon keeps GPU/providers/state warm, but closes (destroys) the window on
     hide and recreates it on toggle. Do not rely on `show()` for focus on Wayland.
   - The daemon owns the state file; the client sends `toggle`/`quit`.
4. Do not build both paths in parallel. The one-shot path is the fallback and the
   test harness for the daemon.

## Consequences

- One-shot is simpler, matches rofi, and is easy to test.
- A daemon adds lifecycle, socket, crash-recovery and stale-socket handling; only
  worth it if the measurement says so.
- The UI/providers are independent of the process model; the window lifecycle is the
  only difference.

## Revisit when

- The real gofi (desktop parsing + ranking + icons) exceeds **400 ms p95** to
  interactive under nested Sway.
- The owner reports the launcher feels slow even when cold start is within budget
  (then the bottleneck is elsewhere, e.g. window mapping or compositor animation).
- Focus-on-reopen becomes unreliable enough to need a resident process.

## Update 2026-11-12

One-shot gofi measured **140 ms median ready, p95 150 ms** under nested Sway
after overlapping the mode provider with window/GPU setup and starting the GPU
prewarm before `App::run` (was 170/180 ms). Rofi's full frame is 58 ms. The
remaining gap is Rune/wgpu window creation and the first draw, so the resident
daemon is the next lever if the owner wants rofi parity; it is not built yet.
