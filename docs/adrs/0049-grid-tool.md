# 0049. The grid tool is one sim command planned edge by edge

- Status: Accepted (M4.5 slice 3); implemented
- Date: 2026-11-06
- Extends: [0038](0038-sim-owned-drawing-guides.md), [0034](0034-command-log-replay-tests.md),
  [0036](0036-money-build-costs-taxes-upkeep.md)

## Context

M4.5 slice 3 lays out many blocks at once: pick a cell size, set the angle with a first edge,
then stretch the area in whole blocks. Each line must join and split existing roads as a
hand-drawn road does (`snap::plan`), overlapping lines are left out, the preview must show
exactly what is built, and a grid dragged over a town's edge must leave no slivers.

Today every road build is one `BuildRoad` or `BuildCurve` with sim-guided coordinates
(ADR 0038). `snap::plan` refuses a whole road for an overlap (a leg within 8 m of a road at
less than about 11.5°), a leg under 10 m, or a road crossed twice; ends within 15 m join a
junction or road. A build is all or nothing on money (ADR 0036). A straight line along a
road follows it and takes the drawn class (ADR 0048). Undo (ADR 0039, proposed) treats one
command as one edit. Lots reach 64 m from a road, are about 16 m wide and merge below 200 m².

## Decision

- **One command.** `Command::BuildGrid { ox, oy, ux, uy, cell, cols, rows, class }`: origin,
  unit axis along the first edge, cell size in metres and signed block counts (the sign picks
  the side; the second axis is the axis turned 90° counter-clockwise). Lattice point
  (i, j) is `o + i·cell·u + j·cell·v`, computed with multiply and add only. The frontend does
  not send many `BuildRoad`s: separate commands could not be refused together for money,
  each would be its own undo step, and later lines would be planned by the browser against
  roads it does not own.
- **The worker builds the command from raw input**, as for roads (ADR 0038): the first edge's
  two raw `[x, y, snap]` points go through `guide::draft`, so the origin joins junctions and
  roads and the angle snaps to the road it starts on or to guide lines; that is how a grid
  lines up with a town. The area cursor `[x, y]` is projected on the guided axes and rounded
  to whole blocks, at least one each way. The command records the guided origin and axis, so
  replays never re-run guide rules.
- **Planning, in order.** Each lattice edge (one cell long) is a candidate straight road.
  1. *Sliver clearance.* A **sliver** is a strip narrower than 40 m (5 zone cells) between
     two roughly parallel roads, centreline to centreline: its lots would be shallower than
     16 m, less deep than one lot is wide. Where existing active roads run alongside a
     candidate edge (within about 23.5°, since planning can still turn a leg by about 12° when
     joining its ends) closer than 40 m for at least 16 m, one lot's width, the edge gives
     way: it is left out if they run beside all of it, its middle, or an end and then turn
     away, and the existing roads take its place; it is cut back to where they stop if they
     end beside it.
  2. *Meet the road that replaced an edge.* A candidate ending on a left-out edge's lattice
     point is extended or cut along its own line to that edge's existing road, so it ends in
     a junction instead of a dead end or a short spur past the road.
  3. *Plan like a hand-drawn road.* Remaining edges, all along `u` row by row and then all
     along `v` column by column, each in increasing index, are planned with `snap::plan`
     against the network plus the grid edges already accepted. An edge the planner refuses
     (overlap, short leg, double crossing) is left out; the others still build. Crossings
     split existing roads, and ends near junctions join them, as usual.
  4. *No slivers, ever.* An edge is also left out if any new leg its plan adds would run
     alongside a road (within 11.5°) closer than 40 m for at least 16 m.
  Straight joins (ADR 0040) then merge collinear edges into one road between junctions.
  Planning runs on a scratch copy of the road network and changes no world state.
- **Class.** `Street` or `Main`; `Alley` is refused, since alleys are laid by the sim. The
  grid never changes an existing road's class: edges along existing roads are left out by
  rule 1 before ADR 0048's follow-and-upgrade could apply. Upgrades stay with drawing and
  slice 4.
- **Cell size** is any value from 40 to 200 m; the sim does not require a multiple of the
  8 m zone cell. Lots come from road geometry, not zone cells, and a rotated grid cannot line
  up with the axis-aligned zone grid anyway. The menu offers five sizes, all multiples of 8 m so
  axis-aligned grids fall on whole zone cells and match length snapping: 48, 64, 80 (the
  default), 96 and 112 m.
- **Money is all or nothing.** The cost is the sum of every accepted edge's new legs at the
  class rate. If the balance cannot cover it the whole grid is refused, as a road is.
  Partial builds were rejected: the preview could no longer show exactly what is built.
- **Limits.** At most 20 blocks along each axis (840 edges). Larger counts are clamped when
  derived from the cursor and refused in a command. Planning costs about edges × active
  roads segment tests, a few million at the limit; previews are revision-tagged so stale
  answers are dropped.
- **Preview contract.** `previewGrid { revision, class, cell, points, cursor }` (points: the
  first edge's two raw `[x, y, snap]` records; cursor: `[x, y]` or absent while the first
  edge is drawn) is answered by `gridPreview` with the road draft fields `points`, `guides`,
  `length`, `angle`, `angles` and `revision`, plus `cols`, `rows`, `roads`
  (`[ax, ay, a_join, bx, by, b_join]` per accepted leg, joins as `stops`: 0 new end,
  1 existing node, 2 splits a road), `dropped` (`[ax, ay, bx, by]` per left-out lattice edge,
  drawn faintly), `cost` (null when nothing is planned) and `affordable`. `buildGrid` with
  the same fields and an `id` runs the same function and applies the command; it answers
  `buildResult`. WASM gains `grid_draft` and `build_grid_draft`, which only convert types.
- **Replays and saves.** Command logs gain `<tick> roadgrid ox oy ux uy cell cols rows class`
  (not `grid`, which already names the start line). Older logs read the same, so
  `LOG_VERSION` stays 1, as when ADR 0036 added `map`. Saves hold the resulting roads, so the
  save format does not change; the drag state is not saved.
- **Undo.** One `BuildGrid` is one road edit in the history ADR 0039 proposes: its added
  roads and splits are undone together.

## Consequences

- Preview, quote and build share one Rust planner, and councilors (M6) can lay grids with an
  ordinary command.
- A grid crossing a town re-cuts its lots and removes buildings in the way, like a hand-drawn
  road. Edges along town roads are left out, so the grid uses the town's roads as its border.
- Existing roads crossing the grid at a shallow angle can still leave small triangles; lots
  below 200 m² merge into neighbours, which hides most of them. The grid only lines up with a
  town's cross streets if the first edge starts on their line (guides help); otherwise the
  offset is the player's choice, and offsets under 40 m are visible as jogs.
- Existing fixtures are unaffected. Add a fixture of a grid dragged over a small town's
  edge and a parser round-trip test. Tests should assert no two near-parallel roads closer
  than 40 m after the build, which is the "no slivers" check for the done-when.
- The 40 m sliver width, the 20-block limit and the cell size range are tuning values, but
  changing them changes what recorded `roadgrid` commands build, so fixtures need blessing.
