# 0041. Power grids follow roads and gate growth

- Status: Accepted; pollution from coal superseded by [0044](0044-coal-plants-pollute.md)
- Date: 2026-11-04
- Extends: [0025](0025-rci-demand-gates-growth.md), [0033](0033-single-city-state-saves.md)

## Context

ROADMAP M5 adds services, and power comes first. It must give the player a reason to place
plants and plan a network without making sandbox fail. It was designed before money and
pollution landed on main; integration preserves those systems and keeps power free. The sim already has a day clock derived from the tick, a
growth gate on R/C/I demand (ADR 0025) and one versioned save format (ADR 0033).

## Decision

- **Grids.** Every road conducts power. A grid is a set of roads joined by road or by
  transmission line, together with the plants and buildings attached to them. Grids are
  derived from roads, lines and plants, not stored.
- **Plants** are placed by the player, not grown by demand. Placing one clears the zone
  cells under its footprint and removes any building there, as a road edit does.
  - Coal: large, constant output. It must touch a road, has industrial jobs (counted in ADR
    0025's industrial demand) and shows smoke with no other effect yet.
  - Solar: output follows the day clock, zero at night. No jobs and no road needed.
  - Wind: output varies smoothly over hours from a signal computed from the seed and tick,
    with no stored state. No jobs and no road needed.
  - Output never depends on staffing.
- **Transmission lines** are straight spans between poles. Their endpoints join plants,
  roads or other lines. They exist to bring in off-road plants and to join grids that roads
  do not connect, and they have no capacity limit. A line's corridor removes the buildings
  whose footprints it crosses and stops lots it crosses from building while it stands;
  zones stay painted. Lines can be bulldozed.
- **Demand** for power comes from each building's homes and jobs, with industrial jobs
  weighted higher. Plants need no power.
- **Allocation.** Each grid serves its buildings oldest first, by building ID, until its
  supply runs out; the rest are unpowered. **Starter power** is one small supply for the
  whole city, used before any plant's, and given to the oldest buildings wherever they
  stand, so separate road networks cannot multiply it.
- **Effect.** A lot builds only if it would be powered, in addition to ADR 0025's demand
  gate. An unpowered building keeps its residents and workers, but its vacant homes and jobs
  do not fill until power returns. Power never closes or removes a building.
- **Waiting warning.** A scheduled lot remembers whether its last growth check failed for
  lack of power. This reason is saved and hashed with the construction schedule, so loading
  preserves the warning. Unzoning, rezoning or removing the schedule clears it; growth
  checks update it as demand and supply change.
- Plants and lines are authoritative state and enter the save and state hash. The save
  version is bumped and older saves and shared links are refused, as ADR 0033 already
  requires; no conversion is written.

## Consequences

Players build roads and a plant before a town grows beyond starter power. The newest
buildings on a solar grid go dark each night but lose no one. Coal is the best plant until
money and pollution add its costs. Grids and allocation must be recomputed when roads,
lines, plants or buildings change and when variable output changes, not every tick. Every
city saved before this change stops loading. Line corridors depend on building footprints
and lot outlines but do not change block or lot generation (ADR 0016, ADR 0018). Commands,
snapshot formats for plants, lines, power state and the overlay, and their frontend readers
are fixed when this is implemented and listed in `CLAUDE.md`.

## Integration note

Money (ADR 0036) and pollution (ADR 0037) landed before this branch. Power remains free to
build and maintain, without new pollution sources. Growth must pass both the land-value
threshold and power allocation. Save version 4 holds all three systems; older versions are
refused. The planned display-only coal smoke is not yet implemented. Road-facing placement
and line terminals extend this decision in [0042](0042-roadside-pieces-snap-to-roads.md).
