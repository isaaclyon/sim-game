# 0036. Drawing guides are planned by the sim and travel as raw points with snap flags

- Status: Accepted
- Date: 2026-11-04

## Context

M4.5 adds angle snapping, guide lines and length snapping to the road tool. Each one moves a
clicked point before it becomes a road. ADR 0003 makes the sim the source of truth, and
ADR 0015 already keeps construction previews sim-owned so a preview always matches the built
road. If the browser moved points itself, snapping logic would be split between TypeScript
and Rust, previews could disagree with builds, and councilors (M6) could not reuse it.

A clicked point's snapping also depends on the points before it: angles are measured from the
road the start joins, or from the start-to-control direction for a curve's end.

## Decision

- `sim::guide::draft` turns raw points into guided points. Input is `[x, y, snap]` per point,
  in drawing order (start, then a curve's control, then the end or cursor). `snap` is a
  bitmask: 1 = angle, 2 = guide lines, 4 = length. Each point is guided from the guided
  points before it. Joining existing junctions and roads is not optional.
- Road previews and builds send those raw points. The worker asks the sim to guide and plan
  them in one call, so a preview and the build from the same points cannot differ.
- Commands are unchanged: the worker applies `BuildRoad` / `BuildCurve` with the guided
  coordinates, so command logs (ADR 0034) and saves (ADR 0033) record final positions, and
  replaying a log never re-runs guide rules. Guide tuning can change without breaking replays.
- The road preview answer adds `points` (`[x, y]` per guided point), `guides` (`[x0, y0, x1, y1]`
  per guide line used for the last point, from the line's origin to the point), `length`
  (metres along the road drawn so far) and `angle` (degrees from the road the start joins, NaN
  when there is none). The build answer adds the guided end, which the next chained road
  starts from.
- Snapping uses fixed unit vectors for 15° steps, not trigonometry, and reads no wall clock or
  random state. The readout angle is display only.

## Consequences

- One Rust implementation serves previews, builds, tests and future councilors.
- Every snapped point costs a worker round trip, as previews already do. If that feels slow,
  the browser may predict a point while it waits, but the sim's answer always wins.
- The previous `previewSnap` / `previewCurve` messages and the browser-drawn curve control
  guide are replaced by one `previewRoad` message carrying points. Rust writers, `protocol.ts`,
  the worker, the tool preview and the project instructions change together.
