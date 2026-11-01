# Architecture Decision Records

Short records of decisions that shape the codebase. Each one says what we decided, why, and what
it costs us. When a decision changes, add a new ADR that supersedes the old one rather than
rewriting history; mark the old one `Superseded by NNNN`.

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-rust-sim-with-browser-frontend.md) | Rust sim compiled to WASM, browser frontend | Accepted |
| [0003](0003-sim-and-rendering-are-separate.md) | Sim and rendering are separate: commands in, snapshots out | Road snapshot cardinality superseded by 0015, its speed field by 0016; otherwise accepted |
| [0004](0004-deterministic-fixed-timestep-sim.md) | Deterministic fixed-timestep sim | Accepted |
| [0005](0005-data-oriented-storage.md) | Data-oriented storage with append-only ids | Accepted |
| [0006](0006-agents-pathfinding-and-traffic-model.md) | Agents, pathfinding and traffic model | Accepted; pathfinding superseded by 0010 |
| [0007](0007-three-js-rendering-approach.md) | Three.js rendering approach | Deprecated |
| [0008](0008-permissive-licenses-only.md) | Permissive-license dependencies only | Accepted |
| [0009](0009-intersection-rules.md) | Intersection rules | Accepted; ms/tick target superseded by 0011 |
| [0010](0010-congestion-aware-routing-and-detours.md) | Congestion-aware routing and detours | Accepted; long-trip search narrowed by 0016 |
| [0011](0011-absolute-tick-budget.md) | Absolute tick budget for traffic | Accepted |
| [0012](0012-la-scale-target.md) | Scale target: Los Angeles at rush hour, 10x speed | Accepted |
| [0013](0013-road-deletion-tombstones.md) | Road deletion uses tombstones | Bulldoze preview shape superseded by 0015; otherwise accepted |
| [0014](0014-zone-grid.md) | Zones live on a fixed grid, independent of blocks | Accepted |
| [0015](0015-curved-road-snapshots.md) | Curved roads use simulation-sampled segment snapshots | Accepted; road speed field superseded by 0016 |
| [0016](0016-road-classes-and-main-road-routing.md) | Road classes and main-road routing | Accepted |

ADRs are only for decisions that are hard to reverse. See [AGENTS.md](AGENTS.md) for when to
write one and how.
