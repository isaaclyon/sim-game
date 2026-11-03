# 0029. Building inspection is a read-only query for one selected building

- Status: Accepted
- Date: 2026-11-02
- Extends: [0022](0022-citizen-inspection-query.md), without changing citizen query semantics

## Context

Players need to understand how much housing and employment each building supplies. The
existing citizen inspector already streams one read-only subject. Building snapshots pack
only active geometry, so a packed index is not a stable BuildingId after demolition. Picking
the ground behind a tall building also misses the roof or facade the player actually clicks.

## Decision

- The existing inspector selects one citizen or one building, never both. With no tool active,
  cars and walkers take priority; otherwise the frontend picks actual building mesh triangles.
  Roof, facade and trim triangles map back to the building that produced them.
- Each worker `buildings` message carries a parallel `Uint32Array` of stable building IDs,
  one per record in the same geometry message. The packed float geometry format remains
  unchanged. IDs come from Rust's active-building iterator in exactly the snapshot order;
  geometry tiles retain these IDs even when unchanged geometry is reused.
- `inspectBuilding` selects a stable ID. The worker queries `World::building_info` at once,
  paused or not, then after each frame and edit. `building` replies echo the same selection
  counter used by citizen inspection, so late replies cannot replace a newer selection.
  No selected subject means no inspector queries. A removed or unknown ID answers null.
- The simulation reports original use, capacity, occupied count and closure status. Residential
  capacity sums all home capacities in the building; occupancy counts household members who
  live there, even when out. Workplace capacity is total jobs and occupancy is assigned workers,
  not physical attendance. Unfilled jobs are total minus filled; they are not a promise of
  accepting new workers while closing or disconnected. Counts do not include visitors.
- Queries take `&self`, use no RNG and change neither state nor hashes. Residential queries use
  the existing sorted home-by-building ranges instead of scanning the entire population.
- Demolition clears selection. Closing buildings keep their original type and show a warning
  that they accept no new residents or workers. Buildings have no Follow action. No resident
  lists, employee lists or links between inspectors are included.

## Consequences

The extra snapshot payload is four bytes per active building, only when geometry is sent.
Counts stream for one selected building, not every building each frame. No gameplay state,
capacity formulas or per-tick work changes. Selection survives packed geometry changes and
cannot silently move to another building after deletion. Very recent growth may remain
unpickable until the existing throttled geometry snapshot arrives; a deleted rendered building
can briefly remain visible, but querying its stable ID cannot select its replacement.
