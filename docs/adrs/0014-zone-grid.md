# 0014. Zones live on a fixed grid, independent of blocks

- Status: Accepted
- Date: 2026-11-01

## Context

The zoning brush (issue #14) lands before block detection and lot subdivision (issue #15). The
roadmap says zoning "paints whole blocks", but blocks don't exist yet, and they change every
time a road is built, split or bulldozed. The sim needs somewhere to keep what the player
painted that survives road edits, that #15 can read, and that saves (M4) can store.

Alternatives considered:

- Zone whole blocks: blocks would have to exist first, and a road edit that splits or merges
  blocks would have to decide what happens to their zones.
- Store each stroke as a shape: exact edges, but overlapping strokes and unzoning need polygon
  clipping.
- Zone road frontage (Cities: Skylines style): conflicts with #15's block-filling lots and
  interior lots reached by alleys.

## Decision

- The world owns a zone grid: one byte per square zone cell, 8 m on a side, stored row-major in
  one `Vec<u8>`. It covers a fixed square fixed when the world is created: the starting road
  network's bounds plus a 480 m margin, aligned to whole cells. Painting outside it is clipped.
- Cell values: 0 = none, 1/2 = residential low/high, 3/4 = commercial low/high, 5/6 = industrial
  low/high. Density is part of the zone value and only the player changes it.
- `PaintZone { ax, ay, bx, by, zone }` sets every cell whose centre lies inside the axis-aligned
  rectangle with those corners. Zone 0 unzones. It returns whether any cell changed.
- Zones may cover roads. Roads don't clear zones, and building or bulldozing a road leaves the
  grid untouched. #15 decides which cells count, using only cells inside blocks.
- The zone grid is part of `state_hash`.
- Snapshot: the whole grid as bytes, plus `[origin_x, origin_y, cell_size, cols, rows]`, sent
  when the grid's version changes.

## Consequences

Zoning works now and needs no block detection. Road edits never lose painted zones. #15 derives
each lot's zone from the cells it covers (for example, the most common one), so a lot edge
along a curved road is only as exact as 8 m cells. The map edge is fixed at creation; roads
built past it can't be zoned until a later change grows the grid (a new ADR). The grid costs
about 1 MB on the default map, sent whole on each paint; dirty-rectangle snapshots can replace
that without changing this decision. Saves (M4) must store the grid, or the command log that
built it.
