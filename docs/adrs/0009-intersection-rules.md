# 0009. Intersection rules

- Status: Accepted
- Date: 2026-10-30

## Context

ADR 0006 left intersections without rules. A car at a lane end jumps straight to the start of its
next lane if that lane has room at its tail. It spends no time in the junction and never
interacts with crossing traffic. Rings of full lanes then wait on each other forever, and the
60 s stuck despawn clears thousands of cars per sim-hour at 50k agents.

M1 needs roads that hold up under a growing city, and junctions that look controlled. Players do
not place controls yet.

## Decision

- **Cars occupy junctions.** Each node with 3+ roads is a junction. A movement (in-lane →
  out-lane) has a short path through it and crosses a fixed set of conflict points, derived from
  road angles when the topology changes. A crossing car reserves its conflict points for its
  crossing time. Movements that share no conflict point run at the same time.
- **Don't block the box.** A car enters a junction only after reserving space at the tail of its
  exit lane. Cars never stall inside a junction.
- **Control is automatic, chosen by shape:**
  - 2 roads: no control.
  - 4+ roads where any road is an avenue: **actuated signal**.
  - Otherwise **priority**: the bigger road has right of way; between equal roads, yield to the
    right. If every approach is waiting, the car that has waited longest goes, then lowest lane id.
- **Gap acceptance with patience.** A yielding car goes when the conflicting traffic leaves a
  big enough gap. The gap it needs shrinks the longer it waits, so side roads cannot starve.
- **Left turns** yield to oncoming traffic. Right turns on red are allowed after yielding, using
  the same gap check.
- **Signals** pair opposite approaches (N+S, then E+W). With 5+ roads, each approach gets its own
  phase. Green has a minimum and a maximum, is extended while cars keep arriving, and an
  empty approach's phase is skipped. A protected-left phase is added only when left-turners have
  waited past a threshold.
- **Determinism.** Junctions, reservations and signal state live in index-addressed `Vec`s
  (struct-of-arrays, append-only `JunctionId`). Every check is resolved in a fixed order (wait
  time, then lane id). No wall clock, no hash-order iteration.
- **Snapshot contract.** A new signal stream, one entry per signalised approach:
  `[x, y, heading, state]`, with state 0 = red, 1 = green, 2 = protected-left green. The Rust
  writer, `protocol.ts` and `renderer.ts` change together.
- **The stuck despawn stays** as a backstop. Each one is counted as a rule failure, not normal
  flow.
- The junction model leaves room for player-placed controls later (a per-junction override of
  the shape-chosen control), but that is out of scope until M5.

## Consequences

- Done when: under 10 stuck despawns per sim-hour at 50k agents, on the generated grid and on a
  scripted hand-drawn layout, enforced by a bench assertion. ms/tick in `traffic_bench` may rise
  by up to 30% over the pre-change baseline.
- Rules make full-lane rings rare but cannot rule them out. That is a capacity problem, which
  the despawn still handles.
- Junction geometry (conflict points, crossing paths) has to be rebuilt when roads change, and
  later when curves and road meshes land. It should share their geometry queries, not duplicate
  them.
- Single-lane roads only: no turn lanes, so a waiting left-turner blocks the cars behind it.
  Multi-lane roads (M6) will need their own extension of this ADR.
- Replaces the "Known gap" consequence of ADR 0006. The rest of 0006 stands.
