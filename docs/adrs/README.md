# Architecture Decision Records

Short records of decisions that shape the codebase. Each one says what we decided, why, and what
it costs us. When a decision changes, add a new ADR that supersedes the old one rather than
rewriting history; mark the old one `Superseded by NNNN`.

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-rust-sim-with-browser-frontend.md) | Rust sim compiled to WASM, browser frontend | Accepted |
| [0003](0003-sim-and-rendering-are-separate.md) | Sim and rendering are separate: commands in, snapshots out | Road snapshot cardinality superseded by 0015, its speed field by 0017; otherwise accepted |
| [0004](0004-deterministic-fixed-timestep-sim.md) | Deterministic fixed-timestep sim | Accepted |
| [0005](0005-data-oriented-storage.md) | Data-oriented storage with append-only ids | Accepted; agent removal completed by 0021 |
| [0006](0006-agents-pathfinding-and-traffic-model.md) | Agents, pathfinding and traffic model | Accepted; pathfinding superseded by 0010; citizen lifecycle and recovery by 0021 |
| [0007](0007-three-js-rendering-approach.md) | Three.js rendering approach | Deprecated |
| [0008](0008-permissive-licenses-only.md) | Permissive-license dependencies only | Accepted |
| [0009](0009-intersection-rules.md) | Intersection rules | Accepted; ms/tick target superseded by 0011 |
| [0010](0010-congestion-aware-routing-and-detours.md) | Congestion-aware routing and detours | Accepted; long-trip search narrowed by 0017 |
| [0011](0011-absolute-tick-budget.md) | Absolute tick budget for traffic | Accepted |
| [0012](0012-la-scale-target.md) | Scale target: Los Angeles at rush hour, 10x speed | Accepted |
| [0013](0013-road-deletion-tombstones.md) | Road deletion uses tombstones | Bulldoze preview shape superseded by 0015; citizen recovery by 0021; otherwise accepted |
| [0014](0014-zone-grid.md) | Zones live on a fixed grid, independent of blocks | Accepted |
| [0015](0015-curved-road-snapshots.md) | Curved roads use simulation-sampled segment snapshots | Accepted; road speed field superseded by 0017 |
| [0016](0016-blocks-lots-and-alleys.md) | Blocks are derived road faces; lots are zone-cell sets; the sim adds alleys | Accepted; lots superseded by 0018 |
| [0017](0017-road-classes-and-main-road-routing.md) | Road classes and main-road routing | Accepted |
| [0018](0018-lots-cut-along-roads.md) | Lots are shapes cut along roads; buildings take their shape from lots | Industrial frontage and buildings superseded by 0019; otherwise accepted |
| [0019](0019-industrial-lot-variety.md) | Industrial lots combine neighbouring frontage strips | Industrial footprint fallback superseded by 0020; otherwise accepted |
| [0020](0020-rectangular-industrial-halls.md) | Industrial halls stay rectangular | Accepted |
| [0021](0021-citizens-households-and-trips.md) | Citizens, households, homes, jobs and trips | Accepted; residents of repainted buildings superseded by 0023 |
| [0022](0022-citizen-inspection-query.md) | Citizen inspection is a read-only query for one selected citizen | Accepted |
| [0023](0023-gradual-closure-of-rezoned-buildings.md) | Gradual closure of rezoned buildings | Accepted |

ADRs are only for decisions that are hard to reverse. See [AGENTS.md](AGENTS.md) for when to
write one and how.
