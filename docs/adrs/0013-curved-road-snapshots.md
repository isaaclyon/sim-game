# 0013. Curved roads use simulation-sampled segment snapshots

- Status: Accepted
- Date: 2026-11-01

## Context

ADR 0003 defines one five-float snapshot record per road. A Bézier centreline needs more than
one line segment, but asking the frontend to evaluate curves would duplicate the geometry
used for driving, snapping and construction previews. Road and lane ids must remain stable
under ADR 0005; rendering samples are not additional roads or junctions.

## Decision

- Replace only ADR 0003's road snapshot cardinality: `[ax, ay, bx, by, speed_limit]` is now
  **per rendered segment**, not per road. Straight roads emit one segment; curves emit several.
  The sim samples them and the worker still sends them only when `roads.version` changes.
  Snapshot record indices are not road ids.
- Construction previews remain simulation-owned `[x, y, join]` samples. Join `-1` identifies
  interior curve samples without a junction marker; `0`, `1` and `2` keep their existing
  meanings (new end, existing node, split road). An empty preview means refused.
- `BuildCurve` supplies start, one quadratic Bézier control point and end through the same
  command boundary as `BuildRoad`. Both use the same construction planner as their previews.
- Keep the rest of ADR 0003: the sim owns state, the frontend sends commands and draws
  snapshots, and the worker owns the clock. Agent and signal snapshot formats are unchanged.

## Consequences

- Rendering, preview and simulation geometry cannot disagree because of separate curve
  implementations. The frontend does not need a Bézier dependency.
- Curved networks send more road and preview records, but do not add graph nodes for sampling.
- Picking a road for future editing cannot infer its id from a rendered segment's index;
  it must query the sim or use a future explicitly identified snapshot format.
- The Rust writers, TypeScript protocol comments, renderer and project instructions change
  together. This partially supersedes ADR 0003, not its simulation/rendering separation.
