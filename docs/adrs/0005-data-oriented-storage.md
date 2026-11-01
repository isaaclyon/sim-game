# 0005. Data-oriented storage with append-only ids

- Status: Accepted; agent removal contract completed by [0021](0021-citizens-households-and-trips.md)
- Date: 2026-10-30

## Context

At 50k-200k agents, per-object allocation and pointer chasing dominate cost. Most systems touch
a few fields of many entities, which favors contiguous arrays per field.

## Decision

- Large collections use struct-of-arrays: one `Vec` per field (`Agents`, `RoadNetwork`).
- Ids (`NodeId`, `LaneId`, `AgentId`) are plain `u32` indices into those arrays.
- Ids are append-only. Nothing is reordered or compacted, so an id stays valid for the life of
  the world. Removal, when we need it, will use tombstones or a free list, not swap-remove.
- Two-way road `k` owns lanes `2k` (a→b) and `2k+1` (b→a).
- Road adjacency is a CSR array, rebuilt lazily when roads change.
- No ECS library for now. Plain arrays are enough and easier for agents to read.

## Consequences

- Fast iteration and cheap snapshots (indices map straight into snapshot buffers).
- Adding a field means adding a `Vec` and keeping it in sync in `push`. Easy to forget; tests help.
- Deleting things (roads, agents) needs a design pass before we support it.
- If entity kinds multiply (buildings, services, vehicles of several types), revisit an ECS
  (e.g. `hecs` or `bevy_ecs`) in a new ADR.
