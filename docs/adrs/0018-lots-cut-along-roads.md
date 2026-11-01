# 0018. Lots are shapes cut along roads; buildings take their shape from lots

- Status: Industrial frontage and building rules superseded by [0019](0019-industrial-lot-variety.md); otherwise accepted
- Date: 2026-11-03

## Context

ADR 0016 made lots sets of 8 m zone cells. On roads at an angle to the grid, and on curves,
lot edges become staircases. Fitting a building into a staircase gave thin or tiny
rectangles at mixed angles (tried in #70 and taken out again). Buildings (#16) should instead
fill lots the way they do in old city centres: wedges on curves, wrap-around buildings on
corners, and growth into the gaps that curves leave. Cities: Skylines stamps fixed rectangles
on road-aligned cells and breaks down exactly there; we want the opposite.

ADR 0016 rejected shaped lots because clipping shapes against irregular blocks is fragile.
`i_overlay` (MIT/Apache-2.0) now handles that: union, intersection, difference and inward
offset of any polygons, including non-convex ones with holes. It snaps coordinates to an
integer grid inside, so results are exact and the same on every machine.

Alternatives considered:

- Keep cell lots and constrain building rectangles (front setback, width and depth limits):
  tidier streets, but staircase lot edges stay and angled or curved lots still get small or
  no buildings.
- Zone cells that follow roads (Cities: Skylines): smooth, but replaces ADR 0014 and zone
  painting, and still forces rectangles at corners and curves.

## Decision

**Zones stay on the 8 m grid (ADR 0014).** Painting, `FillBlock` and zone snapshots are
unchanged. The grid says what land is for; lots say how it is divided.

**A lot is a polygon**, cut from its block's buildable land:

1. The block's land is its outline (ADR 0016) shrunk by half a road width, so it follows
   the kerbs, curves included.
2. Each road side bordering the block is cut into stretches of about 16 m, as now. A
   stretch's lot runs from the kerb straight back from the road, its sides along the road's
   normal at the stretch ends (so lots on curves are wedges; normals are blended between a
   curve's sample points so neighbouring wedges don't cross early). Its back follows the
   line where land stops being nearer this road than any other road (the far side of the same
   road included), traced every 2 m, at most 64 m (the 0016 reach) from the road. Lots from
   facing roads therefore meet on one shared line with no gap, and on the inside of a curve,
   or where a road bends back on itself, wedges meet down the middle.
3. Where two roads meet at a block corner, the angle bisector at that corner divides their
   lots.
4. Each lot is clipped to the block's land. Lots are cut in (road, side, stretch) order; a
   lot gives up any part an earlier lot already holds.
5. A lot is one piece. Pieces smaller than 200 m² (about 40% of a typical lot) join the
   neighbour they share the longest edge with, so slivers at curves and the small triangles
   left at corners become part of a neighbouring lot rather than waste or tiny lots. A small
   piece touching only other small pieces retries once they have joined; if none can join,
   the biggest becomes a lot itself (a whole tiny block, say), so no land is dropped. Gaps
   the lots leave smaller than 1,000 m² (inside curves, between converging wedges) join a
   neighbour the same way. A join that would leave a hole in the lot is skipped; the piece
   tries its other neighbours.
6. Two lots tracing the same shared line from different sample points leave hair-thin strips
   along it. Corners sharper than about 10° that enclose under 2 m² are removed from the final
   outline, so lots have no spikes; the land lost is negligible.

**Rectangle-like blocks** are cut as on a city grid. A block qualifies when it has exactly four
straight sides (each turning less than 30° along its length) and four corners between 60° and
120°, and its shorter pair of opposite sides is at most twice `LOT_REACH` plus a road width
long (136 m), so lots can reach across. Lots face only the longer pair of sides (if the pairs
are within 5%, the more east-west pair). The other two sides are flanks and get no lots:

- Front lots reach back to the line halfway to the facing front; flank roads and roads beyond
  the block's corners don't count in that.
- The end lots of a front run along the flank instead of along a corner bisector.
- A front's stretches are measured along its kerb, between the flanks' kerbs, so the end lots
  are as wide as the rest (a 120 m grid gives seven 16 m lots per side).

A perfect grid of square blocks therefore cuts into identical rectangles, about 16 m by 56 m,
seven along each of two sides of every block. Other blocks keep the rules above.

Land deeper than the lot depth stays the block's interior and drives alleys and connector
streets exactly as in ADR 0016.

**Zone of a lot**: the most common zone among the zone cells whose centre lies inside it, ties
to the lower value. A lot exists only if at least half of those cells are zoned. The per-cell
owner grid (0016) stays, filled by cell centre, so painting and future systems can still ask
"which lot is this cell in".

**Each lot records** its frontage road, front point and facing direction (as now), its polygon
and its frontage length. Ids, tombstones, when lots are cut and recut, and alley rules are as in
ADR 0016. A recut lot keeps its id when its polygon is the same to within 1 cm.

**Buildings (#16) take their footprint from the lot**, by zone and density:

- High density and commercial: the lot shrunk by a small gap from the street, with no gap
  between neighbours, so buildings form a continuous street front that bends with curves and
  wraps corners.
- Low density: a house box facing the road, set back from it, sized to the lot's frontage.
  It does not bend. The rest of the lot is yard, which later procedural detail (gardens,
  fences, trees) can fill.
- Industrial: the lot shrunk by a wider gap, as one large shed.
- If shrinking fails or leaves too little (very odd lots), the building falls back to the
  largest box facing the road, or the lot stays open land.

Exact footprint rules and heights are tuning and belong to #16, not this record.

**Snapshots**: lots become `[n, x0, y0, …]` rings: a point count followed by that many
points, per active lot, replacing the outline segments of ADR 0016. Sent whole when the lots
version changes. Lot polygons are also part of `state_hash`.

## Consequences

- Lot edges follow roads and curves; streets get a regular building line and corners and
  curves get shaped buildings instead of leftovers.
- Adds `i_overlay` as a dependency of `sim`, without its multithreading feature, to keep
  results deterministic. Polygon work costs more than cell sets; cutting must stay near the
  0016 timings (about 2 ms for one block, under a second for the whole default town).
- Zone colour still draws as cells, so a zoned area's edge is a staircase while its lots are
  smooth. The renderer can fill lots in their zone colour over the cells to hide that.
- Lots near the edge of a painted area can include a little unzoned land (up to half), or
  leave a little zoned land unlotted.
- Supersedes the lot shape, lot zone and lot snapshot parts of ADR 0016; blocks, block cells,
  `FillBlock`, alleys, lot ids and recut rules stand.
- As in 0016, changing shape numbers (stretch width, reach, sliver size) changes lots in
  replayed worlds, so they are part of this contract.
