# 0046. Continuous curves use sim-guided continuation drafts

- Status: Accepted
- Date: 2026-11-05
- Extends: [0038](0038-sim-owned-drawing-guides.md)

## Context

A continuous curve must leave each previous piece in its outgoing direction. The original
curve tool leaves its bend free, and road edits can split roads or add alleys, so the newest
road ID cannot reliably identify the previous piece. Preview and build must still agree, and
command logs must keep final geometry rather than depend on future guide rules.

## Decision

- A separate tool draws its first curve with start, control and end clicks. Each later click
  requests an end. The worker returns the sim-guided control of each successful piece to the
  tool; cancelling or replacing the tool discards this transient drawing state.
- A continuation draft has three raw `[x, y, snap]` records: start with bit 8 set, desired end
  with its usual snap flags, and the previous sim-guided control. The sim guides the endpoints,
  checks that an active road at the start has the same outgoing direction, and derives the new
  control in that direction, at half the end displacement's projection on it. It refuses an
  invalid source or a nonpositive projection. The preview returns start, new control and end;
  an invalid continuation returns no points or stops.
- Existing junction snapping remains in force, including the nodes between curved pieces.
  The normal planner can still refuse a piece for overlap, crossings, short joins or cost.
- The resulting curve is built with the existing `BuildCurve` command. Saves and replay logs
  do not store the temporary drawing state or need a new format.

## Consequences

- The sim owns the same geometry for preview, price and construction. No road ID needs to be
  inferred from insertion order, and a changed source direction refuses rather than kinking.
- Continuations need the previous control from the actual successful build, not an old preview.
  A matching direction does not prove road identity if another road shares that direction.
- Tight turns take several shorter pieces; an end behind the outgoing direction is refused.
  Chained curve nodes remain snap targets, as deferred by ADR 0040.
