# 0006. Agents, pathfinding and traffic model

- Status: Accepted; the Pathfinding bullet is superseded by [0010](0010-congestion-aware-routing-and-detours.md)
- Date: 2026-10-30

## Context

Skylines-style games simulate individual citizens, but most citizens are idle at any moment.
Simulating everyone every tick wastes most of the budget. Pathfinding is the next biggest cost
and can spike when many trips start at once.

## Decision

- **Agent lifecycle:** `Parked` → `AwaitingPath` → `Entering` → `Driving` → back to `Parked`.
  Parked agents cost only a wake timer; only trip agents do real work.
- **Pathfinding:** A* on the lane graph, cost = travel time, heuristic = straight-line distance /
  fastest speed limit (admissible). Scratch buffers are generation-stamped so a search doesn't
  clear the whole graph. A per-tick budget (`path_budget`) spreads path requests over ticks.
- **Traffic:** each lane is a FIFO queue (`VecDeque`). Vehicles follow the car ahead with fixed
  spacing (7 m) and a simple accelerate-to-limit rule. A vehicle moves to its next lane only if
  that lane has room at its tail.
- **Gridlock safety valve:** a vehicle stuck 60 s at a lane end is despawned and its agent
  returns to parked (Skylines does the same).
- Roads are straight segments today. `lane_point` and `lane_heading` are the only geometry
  queries, so curves can replace them without changing ids or the traffic model.

## Consequences

- Scales to 50k agents with ~38k driving at once, well inside a frame budget.
- **Known gap** (addressed by [0009](0009-intersection-rules.md)): intersections have no rules. Circular waits cause gridlock (thousands of stuck
  despawns per sim-hour at 50k agents). Next step: intersection reservations or signals, in a new
  ADR.
- Single-lane roads only; no lane changes or multi-lane roads yet.
- Trips are random node-to-node. Real origins and destinations come with zoning and buildings.
