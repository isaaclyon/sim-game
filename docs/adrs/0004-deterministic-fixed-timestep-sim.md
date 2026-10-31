# 0004. Deterministic fixed-timestep sim

- Status: Accepted
- Date: 2026-10-30

## Context

Bugs in a large agent sim are hard to reproduce unless a run can be replayed exactly.
Determinism also makes save files small (seed + command log), enables replay-based regression
tests, and keeps lockstep multiplayer possible later.

## Decision

- `World::tick()` advances a fixed `dt` (0.1 s, 10 Hz). The worker uses an accumulator to run as
  many ticks as real time and the speed multiplier call for, capped at 20 per step; if it falls
  further behind it drops the backlog instead of spiralling.
- All randomness comes from the seeded `Rng` (SplitMix64) in the sim. No wall clock, no
  `Math.random`, no OS entropy.
- Logic that affects state must not depend on `HashMap`/`HashSet` iteration order. Sets are fine
  for membership checks.
- Ties are broken explicitly (e.g. the A* heap orders equal costs by id).
- `World::state_hash` gives a cheap fingerprint for comparing runs. The
  `same_seed_same_city` test guards it.

## Consequences

- Same seed + same commands = same world, on the same build.
- Floats are not guaranteed bit-identical across platforms (native vs WASM, different CPUs).
  Cross-platform lockstep would need fixed-point math or careful auditing; not needed yet.
- Contributors must avoid hidden nondeterminism. Reviewers should watch for it.
