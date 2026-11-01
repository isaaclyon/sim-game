# 0016. Blocks, lots and alleys

- Status: Accepted; lot shape, lot zone and lot snapshot superseded by [0017](0017-lots-cut-along-roads.md)
- Date: 2026-11-02

## Context

Issue #15: zoning paints whole blocks, the sim cuts lots to fill the land between roads
(including odd shapes left by curves), and blocks too deep to reach from their edges get alleys
and connector streets that are ordinary roads. Buildings (#16) will sit on lots, rezoning must
not bulldoze them (#17), and citizens (#18) will live and work on them, so lot identity is a
contract other systems depend on. Zones already live on a fixed 8 m grid (ADR 0014).

Alternatives considered:

- Polygon lots (frontage strips clipped against the block): exact edges, but clipping strips
  against arbitrary non-convex blocks with curved sides, dead ends and islands is fragile.
- Storing blocks with stable ids: every road edit would have to decide which block survives a
  split or merge. Nothing needs block ids yet.
- Lots for every block from the start: simple, but costs time and memory on land nobody zoned
  (about 1M lots at the ADR 0012 scale).

## Decision

**Blocks are derived, not stored.** A block is a bounded face of the road graph: walk active
lanes keeping the face on the left, turning onto the sharpest left exit at each node, along
the sim-sampled centrelines (ADR 0015). Faces with positive area are blocks; the outer face of
each connected piece is dropped. Dead ends inside a block belong to it. Where a separate road
network sits inside a block, its own blocks win (the smallest block containing a point owns
it). Block indices change with every road edit and are never stored or sent as ids.

**Block cells** are the zone cells whose centre lies inside the block and at least 4 m (half a
road) from every road centreline. Cells under roads belong to no block.

**`FillBlock { x, y, zone }`** sets every cell of the block containing `(x, y)` to `zone`
through the zone grid (ADR 0014 storage is unchanged). Cells under the roads around it keep
their zone. A point outside every block changes nothing. It returns whether any cell changed.

**Lots are sets of zone cells.** Each block cell within 64 m of a road centreline belongs to
its nearest road and side of that road; that side is cut into stretches of about 16 m, and the
cells of one stretch form one lot. **A lot is always one piece:** its cells connect through
shared edges. A stretch whose cells fall apart (curves, dead ends) becomes one lot per piece.
Pieces with fewer than two cells (corner leftovers) join a lot of the same block that they
touch, or have no lot if none does. No land within reach is dropped for being oddly shaped.
Deeper cells are the block's interior and have no lot. Lot edges along curves are therefore as
exact as 8 m cells, which ADR 0014 anticipated. Each lot records its frontage road, the
centreline point its own cells front on average, the unit direction it faces (toward that road), its cells
and the bounding box of the block it was cut from. A lot's zone is not stored: it is the most
common zone among its cells, ties going to the lower zone value.

**Lot ids are append-only with tombstones**, like roads (ADR 0013). A per-cell owner grid,
allocated when the first lot is cut, maps each zone cell to at most one active lot.

**When lots change:**

- A zoning command (`PaintZone`, `FillBlock`) that changes cells cuts lots in every block it
  touched that contains a zoned cell and has no lots yet. Zoning removes or recuts lots only in
  a block an alley divides (below), so repainting or unzoning keeps lot ids (and, later, their
  buildings; see #17).
- A road edit (build, split, bulldoze) recuts the blocks near the edit. A recut lot with the
  same cells, frontage road and front point as an existing lot keeps that lot's id (and takes
  its block's new bounds); other old lots there are retired and new ones appended. Touched
  blocks with no zoned cells lose their lots.

**Alleys.** After a zoning command, a block that has at least six zoned interior cells, among
them a cell whose zone this command changed, is cut by a straight road through its middle, ending on
the roads around it. If the block is more than twice as long as it is wide, the cut is an
alley along its length at 30 km/h; otherwise it is a connector street across it at street
speed. The pieces are checked again, up to six rounds per command. Cuts go through the normal
construction planner (`BuildRoad` rules), so they snap, split and refuse exactly like player
roads, and the player can bulldoze them. Road edits never add alleys, so a bulldozed alley
stays gone until the player changes the zone of that block's interior again. Only the divided
block is recut, by the road edit rule: its lots that no longer match are retired, since the
alley may run through them. Lots across the street keep their ids; where an alley splits the
road they face, they switch to the piece their front point lies on.

**Buildings (#16)** will live in their own struct-of-arrays, each on one lot id, at most one per
lot. Retiring a lot (a road edit) removes its building; zoning changes never retire lots.

**Snapshots** (read-only):

- Lots: `[ax, ay, bx, by]` per lot outline segment, along zone cell edges where the owning lot
  changes, sent when the lots version changes.
- Block preview: `[x, y]` per outline sample of the block under the cursor, empty if none.

Lot slot count and active lot ids are part of `state_hash`, because lot ids depend on history.

## Consequences

- Any block shape works, including curves, dead ends and islands, with simple per-cell code.
- Lots appear only where the player zones, so unzoned maps and the traffic bench pay nothing.
  Once lots exist, each road edit traces every block of the network, which is linear in roads.
- The owner grid costs 4 bytes per zone cell once used: about 4 MB on the default map, 40 MB
  at the ADR 0012 scale. A sparse store can replace it without changing this decision.
- Corner lots split along the corner's bisector; isolated single-cell leftovers have no lot.
- Lot ids depend on the order of edits, so saves (M4) must store lots, or the command log.
- Lot zone is coarse at mixed edges; a lot half-zoned and half-unzoned can come out unzoned.
- Changing lot shape rules (widths, reach) changes which cells form which lots; worlds replayed
  from a command log will cut different lots, so treat those numbers as part of this contract.
