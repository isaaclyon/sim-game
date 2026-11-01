# 0013. Road deletion uses tombstones

- Status: Bulldoze preview shape superseded by [0014](0014-curved-road-snapshots.md); otherwise accepted
- Date: 2026-10-31

## Context

Bulldozing needs the removal contract left open by ADR 0005. Reusing an index through a free
list could make a stale lane or road reference point at an unrelated road. Compacting arrays
would break the append-only ID contract.

## Decision

- Keep append-only road and lane slots. Mark a deleted two-way road inactive (a tombstone).
  Never reuse its ID or its two lane IDs. Keep its geometry available for stale references.
- Count and enumerate only active roads for routing adjacency, snapping, picking and road
  snapshots. Lane counts remain allocated slot counts for lane-indexed storage.
- Retain nodes, including isolated nodes: they remain valid trip locations and reusable build
  anchors. Trips without a path wait and retry; deletion does not relocate citizens.
- `DeleteRoad { x, y }` removes the nearest active segment within 15 metres, both directions.
  Equal-distance hits choose the lowest road ID. Misses change nothing. The read-only bulldoze
  preview uses the same selection and returns `[ax, ay, bx, by]`, or an empty buffer.
- Park driving and entering cars whose remaining route uses the deleted road, as for a split.
  Driving cars park at their current lane's destination node and retry after ten sim seconds.
  Unaffected routes continue. Existing junction reservations expire normally; retained lane
  geometry keeps their headings valid. Rebuild adjacency and junction controls immediately.
- Road snapshots pack active roads only; their positions are not road IDs. Hash tombstones as
  part of world state so deletion is visible to replay checks even without moving cars.

## Consequences

Stable references and deterministic replay need no generation counters or ID migration.
Deleted geometry and lane-indexed storage consume memory for the world's lifetime; repeated
editing grows those arrays. A future compaction or reuse scheme needs a new ADR and a migration
contract. This completes, rather than supersedes, ADR 0005's removal decision.
