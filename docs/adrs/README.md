# Architecture Decision Records

Short records of decisions that shape the codebase. Each one says what we decided, why, and what
it costs us. When a decision changes, add a new ADR that supersedes the old one rather than
rewriting history; mark the old one `Superseded by NNNN`.

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-rust-sim-with-browser-frontend.md) | Rust sim compiled to WASM, browser frontend | Accepted |
| [0003](0003-sim-and-rendering-are-separate.md) | Sim and rendering are separate: commands in, snapshots out | Road snapshot cardinality superseded by 0015, its speed field by 0017; agent format by 0026; frame pacing and buffer reuse amended; otherwise accepted |
| [0004](0004-deterministic-fixed-timestep-sim.md) | Deterministic fixed-timestep sim | Accepted |
| [0005](0005-data-oriented-storage.md) | Data-oriented storage with append-only ids | Accepted; agent removal completed by 0021 |
| [0006](0006-agents-pathfinding-and-traffic-model.md) | Agents, pathfinding and traffic model | Accepted; pathfinding superseded by 0010; citizen lifecycle and recovery by 0021 |
| [0007](0007-three-js-rendering-approach.md) | Three.js rendering approach | Deprecated |
| [0008](0008-permissive-licenses-only.md) | Permissive-license dependencies only | Accepted |
| [0009](0009-intersection-rules.md) | Intersection rules | Accepted; ms/tick target superseded by 0011 |
| [0010](0010-congestion-aware-routing-and-detours.md) | Congestion-aware routing and detours | Accepted; long-trip search narrowed by 0017; cost superseded by 0024 |
| [0011](0011-absolute-tick-budget.md) | Absolute tick budget for traffic | Accepted |
| [0012](0012-la-scale-target.md) | Scale target: Los Angeles at rush hour, 10x speed | Accepted |
| [0013](0013-road-deletion-tombstones.md) | Road deletion uses tombstones | Bulldoze preview shape superseded by 0015; citizen recovery by 0021; otherwise accepted |
| [0014](0014-zone-grid.md) | Zones live on a fixed grid, independent of blocks | Accepted |
| [0015](0015-curved-road-snapshots.md) | Curved roads use simulation-sampled segment snapshots | Accepted; road speed field superseded by 0017 |
| [0016](0016-blocks-lots-and-alleys.md) | Blocks are derived road faces; lots are zone-cell sets; the sim adds alleys | Accepted; lots superseded by 0018 |
| [0017](0017-road-classes-and-main-road-routing.md) | Road classes and main-road routing | Accepted; class kept by followed roads superseded by 0048 |
| [0018](0018-lots-cut-along-roads.md) | Lots are shapes cut along roads; buildings take their shape from lots | Industrial frontage and buildings superseded by 0019; low-density commercial footprints by 0031; residential frontage by 0043; otherwise accepted |
| [0019](0019-industrial-lot-variety.md) | Industrial lots combine neighbouring frontage strips | Industrial footprint fallback superseded by 0020; snapshot extended with `front` by 0027; otherwise accepted |
| [0020](0020-rectangular-industrial-halls.md) | Industrial halls stay rectangular | Accepted |
| [0021](0021-citizens-households-and-trips.md) | Citizens, households, homes, jobs and trips | Accepted; residents of repainted buildings superseded by 0023; walking and travel slots by 0026; industrial job-capacity tuning by 0028; low-density commercial capacity by 0030; first-activity timing by 0047 |
| [0022](0022-citizen-inspection-query.md) | Citizen inspection is a read-only query for one selected citizen | Accepted; pedestrian picking superseded by 0026 |
| [0023](0023-gradual-closure-of-rezoned-buildings.md) | Gradual closure of rezoned buildings | Accepted; replacement growth gated by demand in 0025 |
| [0024](0024-per-driver-route-noise.md) | Per-driver route noise | Accepted |
| [0025](0025-rci-demand-gates-growth.md) | R/C/I demand gates growth | Accepted |
| [0026](0026-walking-trips-and-household-cars.md) | Walking trips and household cars | Accepted |
| [0027](0027-display-only-scenery-and-building-kit.md) | Display-only scenery, building kit and agent looks | Accepted |
| [0028](0028-industrial-employment-density.md) | Industrial employment density | Accepted; industrial minimum job counts superseded by 0030 |
| [0029](0029-building-inspection-query.md) | Building inspection is a read-only query for one selected building | Accepted |
| [0030](0030-small-workplace-capacity.md) | Small-workplace employment capacity | Low-density commercial staffing rate superseded by 0032; otherwise accepted |
| [0031](0031-small-low-density-shops.md) | Low-density commerce grows small shops, not full-lot blocks | Accepted |
| [0032](0032-compact-shop-staffing.md) | Compact shops support small teams | Accepted |
| [0033](0033-single-city-state-saves.md) | Save one complete city locally and resume it on return | Accepted; implemented |
| [0034](0034-command-log-replay-tests.md) | Replay regression tests from saved command logs | Accepted; implemented |
| [0035](0035-share-cities-as-forked-copies.md) | Share cities as files and links that open as independent copies | Accepted; implemented |
| [0036](0036-money-build-costs-taxes-upkeep.md) | Money: build costs, taxes and upkeep on an empty starting map | Accepted; implemented; free followed legs superseded by 0048; starting funds and grant amended |
| [0037](0037-pollution-and-land-value-steer-growth.md) | Pollution lowers land value, and land value steers growth | Accepted |
| [0038](0038-sim-owned-drawing-guides.md) | Drawing guides are planned by the sim and travel as raw points with snap flags | Accepted; amended (snap reach follows zoom) |
| [0039](0039-road-edit-history.md) | Road edit history lives in the sim and is saved with the city | Superseded by 0051 |
| [0040](0040-straight-joins.md) | Straight roads that meet end to end in a line become one road | Accepted |
| [0041](0041-power-grids-follow-roads.md) | Power grids follow roads and gate growth | Accepted; free plants superseded by 0052 |
| [0042](0042-roadside-pieces-snap-to-roads.md) | Roadside pieces snap to the road and face it | Accepted |
| [0043](0043-residential-lot-variety.md) | Residential lots mix standard and wider frontages | Accepted |
| [0044](0044-coal-plants-pollute.md) | Coal plants pollute | Accepted |
| [0045](0045-bulldozer-demolishes-buildings.md) | The bulldozer demolishes buildings | Accepted |
| [0046](0046-continuous-curves.md) | Continuous curves use sim-guided continuation drafts | Accepted |
| [0047](0047-move-ins-follow-daily-schedules.md) | Move-ins follow daily schedules | Accepted |
| [0048](0048-drawing-over-roads-changes-their-class.md) | Drawing a road over existing roads changes their class | Accepted; implemented |
| [0049](0049-grid-tool.md) | The grid tool is one sim command planned edge by edge | Accepted; implemented |
| [0050](0050-click-to-change-road-class.md) | Clicking a road changes its class | Accepted; implemented |
| [0051](0051-road-edit-history-revised.md) | Edit history records before-and-after shapes and replays them as new edits | Accepted; amended; implemented; supersedes 0039 |
| [0052](0052-water-follows-roads-and-supplies-dense-buildings.md) | Water follows roads and supplies dense buildings | Accepted; implemented |
| [0053](0053-fire-stations-send-trucks-through-traffic.md) | Fire stations send real trucks through traffic | Accepted; implemented |
| [0054](0054-children-births-and-growing-up.md) | Children are citizens who are born, grow up and move out | Accepted; implemented |
| [0055](0055-schools-and-education-levels.md) | Schools teach children, and education shapes who fills jobs | Accepted |

ADRs are only for decisions that are hard to reverse. See [AGENTS.md](AGENTS.md) for when to
write one and how.
