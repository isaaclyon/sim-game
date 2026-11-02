# 0023. Gradual closure of rezoned buildings

- Status: Accepted; replacement growth gated by demand in [0024](0024-rci-demand-gates-growth.md)
- Date: 2026-11-02

## Context

Repainting or unzoning occupied land must not bulldoze it. The old buildings empty out over
time, and the new zone replaces them (ROADMAP M2). ADR 0021 kept residents and employees of a
repainted building until demolition and left gradual closure to this change. Road edits still
remove buildings immediately. Closure must not add idle per-tick scans, and it must stay
deterministic.

## Decision

- A building is closing while its original zone differs from its lot's effective zone
  (`Lots::zone`, ADR 0018). A density change is a mismatch. Original use and geometry stay
  until removal. Closing buildings accept no new household, job or shopping visit, as in
  ADR 0021.
- Closure is decided at the existing edit boundary: after building reconciliation in
  `World::apply`, including while paused. A later repaint to another mismatched zone keeps
  the current schedule. Restoring the exact original zone cancels the remaining steps.
  Departed households and released jobs are not recalled.
- Workers lose their jobs when closure starts. Anyone working there or commuting there
  heads home, so no one keeps a work activity without a job. Shoppers already present or
  travelling there keep their visit while the building stands.
- Closure advances in steps 3–12 sim seconds apart, drawn from the resident RNG. Each step
  relocates one household of the building (ADR 0021: a whole household moves to an eligible
  vacant home or leaves town). The first step that finds no household removes the building.
  Empty buildings and workplaces therefore go at their first step, not at the edit. These are
  tuning values, not configuration.
- Removal releases the lot only for the building that still owns it, then schedules
  construction for the lot's zone at that moment through the existing per-lot timer. The last
  painted zone wins; an unzoned lot stays vacant. Resident cleanup reuses ADR 0021's
  demolition path once per tick: trips to the building end without arrival and people there
  recover home. Building ids are never reused, so stale closure events cannot affect a
  replacement.
- Closure deadlines live in a per-building array with an event heap. Heap entries whose
  deadline no longer matches are stale and skipped. Both are part of the state hash. Building
  removal by closure changes the buildings version like growth, so snapshots follow the
  existing throttle; no snapshot format changes.

## Consequences

This supersedes ADR 0021's rule that residents and employees remain until demolition for
repainted buildings. Road-driven demolition is unchanged.

The edit boundary gains one pass over buildings, like the existing lot pass, and one pass over
citizens when a closure starts. Each closure step costs one household relocation. Relocation
walks a derived index of vacant homes in open buildings in id order, so it picks the same home
as a scan of every home without paying for occupied or closing ones. The index follows from
hashed state, so it is not hashed; tests check it against that state. Each tick with a removal costs one resident
reconciliation over households and citizens. Nothing runs on ticks without a due step.

A quick repaint and undo still fires the building's workers. Closure ignores demand (#22);
replacement growth uses the existing timed placeholder.
