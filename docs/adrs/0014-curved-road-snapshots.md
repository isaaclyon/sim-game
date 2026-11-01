# 0014. Curved roads use simulation-sampled segment snapshots

- Status: Accepted
- Date: 2026-11-01

## Context

ADR 0003 defines one five-float snapshot record per road. A Bézier centreline needs more than
one line segment, but asking the frontend to evaluate curves would duplicate the geometry
used for driving, snapping and construction previews. Road and lane ids must remain stable
under ADR 0005; rendering samples are not additional roads or junctions.

## Decision

- Replace only ADR 0003's road snapshot cardinality: `[ax, ay, bx, by, speed_limit]` is now
  **per active rendered segment**, not per road. Straight roads emit one segment; curves emit several.
  Deleted roads emit nothing, preserving ADR 0013's tombstone and packed-snapshot contracts.
  The sim samples them and the worker still sends them only when `roads.version` changes.
  Snapshot record indices are not road ids.
- Construction previews remain simulation-owned `[x, y, join]` samples. Join `-1` identifies
  interior curve samples without a junction marker; `0`, `1` and `2` keep their existing
  meanings (new end, existing node, split road). An empty preview means refused.
- `BuildCurve` supplies start, one quadratic Bézier control point and end through the same
  command boundary as `BuildRoad`. Both use the same construction planner as their previews.
- Extend ADR 0013's bulldoze snapshot from one endpoint pair to `[x, y]` per centreline sample.
  Straight roads still emit two points. Picking measures distance to the actual curve, not its
  endpoint chord; deletion removes the whole selected road between junctions, in both directions.
  The renderer draws all returned samples so the highlight shows exactly what will disappear.
- Keep the rest of ADR 0003: the sim owns state, the frontend sends commands and draws
  snapshots, and the worker owns the clock. Agent and signal snapshot formats are unchanged.

## Consequences

- Rendering, preview and simulation geometry cannot disagree because of separate curve
  implementations. The frontend does not need a Bézier dependency.
- Curved networks send more road and preview records, but do not add graph nodes for sampling.
- Picking a road cannot infer its id from a rendered segment's index; bulldozing queries the
  sim. Other editing tools must also query the sim or use an explicitly identified snapshot.
- The Rust writers, TypeScript protocol comments, renderer and project instructions change
  together. This partially supersedes ADR 0003's road cardinality and ADR 0013's bulldoze
  preview shape, not simulation/rendering separation or stable deletion IDs.
