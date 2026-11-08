# 0003. Sim and rendering are separate: commands in, snapshots out

- Status: Superseded by [0015](0015-curved-road-snapshots.md) for road snapshot cardinality and by [0017](0017-road-classes-and-main-road-routing.md) for the road speed field, and by [0026](0026-walking-trips-and-household-cars.md) for the agent format; all other decisions remain accepted
- Date: 2026-10-30

## Context

City builders tangle game state with presentation, and that gets expensive at scale: the sim
can't run faster than the frame rate, and UI code reaches into state it shouldn't change. We also
want the option to swap the frontend (ADR 0002) without touching the sim.

## Decision

- The sim owns all game state. The frontend never computes or stores authoritative state.
- The frontend changes the world only by sending a `Command` (e.g. `SpawnAgents`, `BuildRoad`).
  Commands flow `main.ts` → `postMessage` → `sim.worker.ts` → `World::apply`. The message types
  live in `web/src/protocol.ts`.
- The sim exposes read-only snapshots as flat `Float32Array`s, transferred (not copied) to the
  main thread:
  - agents: `[x, y, heading, speed_ratio]` per agent, `speed_ratio < 0` means hidden
  - roads: `[ax, ay, bx, by, speed_limit]` per road, re-sent only when `roads.version` changes
- The worker runs the sim clock. The main thread interpolates between the two latest snapshots
  for smooth motion at any frame rate.

## Consequences

- Sim speed (1x, 10x, paused) is independent of frame rate.
- The sim is testable and benchmarkable headless (`cargo test`, `traffic_bench`).
- Any new UI interaction needs a `Command`, even trivial ones. That is the point.
- Snapshot formats are a contract. Changing one means changing the Rust writer, the
  `protocol.ts` doc comment and the `renderer.ts` reader together.
- Snapshots currently send every agent every tick. At much larger counts we'll need culling or
  delta snapshots.

## Amendment (2026-11-07): frames follow the screen, and agent buffers come back

Where this section and the text above disagree, this section wins.

- The worker no longer sends a `frame` after every step that ran ticks. It sends at most one
  about every 1/60 of a real second, always with the newest state, and one after every edit.
  Ticks between frames only change the next frame. At high speeds this stops the page from
  receiving, and throwing away, snapshots it never draws.
- The page hands each agents buffer it stops drawing back to the worker (`spareAgents`,
  transferred). The worker refills a returned buffer when it is big enough and otherwise makes
  one a quarter larger than needed, so steady play allocates no agent snapshots. A `frame`'s
  agents may therefore be a view into a larger buffer.
