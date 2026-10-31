# Architecture Decision Records

Short records of decisions that shape the codebase. Each one says what we decided, why, and what
it costs us. When a decision changes, add a new ADR that supersedes the old one rather than
rewriting history; mark the old one `Superseded by NNNN`.

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-rust-sim-with-browser-frontend.md) | Rust sim compiled to WASM, browser frontend | Accepted |
| [0003](0003-sim-and-rendering-are-separate.md) | Sim and rendering are separate: commands in, snapshots out | Accepted |
| [0004](0004-deterministic-fixed-timestep-sim.md) | Deterministic fixed-timestep sim | Accepted |
| [0005](0005-data-oriented-storage.md) | Data-oriented storage with append-only ids | Accepted |
| [0006](0006-agents-pathfinding-and-traffic-model.md) | Agents, pathfinding and traffic model | Accepted |
| [0007](0007-three-js-rendering-approach.md) | Three.js rendering approach | Deprecated |
| [0008](0008-permissive-licenses-only.md) | Permissive-license dependencies only | Accepted |

ADRs are only for decisions that are hard to reverse. See [AGENTS.md](AGENTS.md) for when to
write one and how.
