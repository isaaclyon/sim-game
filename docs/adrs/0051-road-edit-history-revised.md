# 0051. Edit history records before-and-after shapes and replays them as new edits

- Status: Accepted; amended 2026-11-07; implemented (M4.5 slice 5)
- Date: 2026-11-06
- Supersedes: [0039](0039-road-edit-history.md)
- Extends: [0013](0013-road-deletion-tombstones.md), [0033](0033-single-city-state-saves.md),
  [0034](0034-command-log-replay-tests.md), [0036](0036-money-build-costs-taxes-upkeep.md),
  [0040](0040-straight-joins.md), [0041](0041-power-grids-follow-roads.md),
  [0042](0042-roadside-pieces-snap-to-roads.md), [0045](0045-bulldozer-demolishes-buildings.md),
  [0046](0046-continuous-curves.md), [0048](0048-drawing-over-roads-changes-their-class.md)

## Context

ADR 0039 proposed many-step undo and redo for road edits, kept in the sim and saved with the
city, but left the record format, money and grouping open. Since then:

- **Straight joins (0040).** After every road build and bulldoze, two straight roads of one
  class meeting in a line at a node with no other road become one: the lower ID is extended in
  place and the other is tombstoned. One edit can therefore *reshape* a road it never touched
  directly, and undoing it must unmerge, then let the join pass re-merge where it should.
- **Continuous curves (0046).** Each piece is its own `BuildCurve`. The chain is transient
  tool state; the sim only sees bit 8 (`guide::CONTINUE`) on a continuation draft's start,
  which `World::draft_command` consumes before building the command.
- **Drawing over roads changes their class (0048)** in place, charging the build-rate
  difference for upgrades and nothing for downgrades. Cars keep their routes.
- **Click to change a road's class (0050, in another branch).** `SetRoadClass { x, y, class }`,
  logged as `class x y street|main|alley`, with 0048's in-place logic and cost rules.
- **Grid tool (0049, accepted, not built).** `BuildGrid { ox, oy, ux, uy, cell, cols, rows,
  class }`, logged as `roadgrid`, builds up to 840 edges and is one undo step.
- **Money (0036).** Builds are all or nothing on cost; bulldozing is free and refunds nothing.
  Plants and lines are free (0041).
- **Power (0041, 0042, 0045).** `PlacePlant` places a plant at a snapped footprint (centre,
  front, kind), unzones the cells under it and removes the buildings it covers. `BuildLine`
  snaps both ends and removes buildings its corridor crosses. Grids are derived from roads,
  lines and plants. `DeleteRoad` removes a plant, else a line, else a building, else a road.
  Bulldozing a plant leaves its cells unzoned.

What the code does today:

- `World::build_road` (`world/roads.rs`) plans with `snap::plan`, charges `money::plan_cost`,
  then `add_plan` adds nodes, splits crossed roads, adds legs (`add_shape`) and sets followed
  roads' class (`set_class`). Then `join_straight_roads`, `sync_roads`, `remove_cars_using` and
  `update_lots(Scope::Near(rect), true)`.
- `update_lots` can cut alleys and connector streets into deep blocks through the same
  `add_road` path (`world/zoning.rs`). It runs after road edits **and after zoning**, so roads
  can change outside any player edit.
- Every road change goes through five `RoadNetwork` primitives: `add_shape`, `split_road`,
  `remove_road`, `join_straight` and `set_class`. Power changes go through `Power::add_plant`,
  `add_line`, `remove_plant` and `remove_line`. All IDs are append-only with tombstones.
- Saves are postcard of the whole `World` behind `SAVE_VERSION` 4 (`world/persist`). Command
  logs are versioned text (`replay`); the WASM `Sim` records every command, refused ones too.

## Decision

### Scope

History covers every placeable built with a build tool: roads (`BuildRoad`, `BuildCurve`,
`BuildGrid`, `SetRoadClass`, and a `DeleteRoad` that removed a road), power plants and lines
(`PlacePlant`, `BuildLine`, and a `DeleteRoad` that removed a plant or line). Later placeables,
such as the planned water pumps, join the same history as new record kinds.

Not covered: zone painting and block fill, building demolition, and spawning. Buildings an edit
or bulldoze removed stay gone, as ADR 0045 says.

### What a step records: before-and-after shapes, not inverse operations

- A step holds `before` and `after`: lists of records, plus `kind` (road build, curve, grid,
  class change, road bulldoze, plant, line, plant bulldoze, line bulldoze) and `cost` (whole
  dollars the step charged; 0 for bulldozes, downgrades, plants and lines).
- Records, positions in `f32` as stored:
  - road `{a, b, control, class}`;
  - plant `{kind, centre, front}`, the placed footprint, not the click;
  - line `{a, b}`, the snapped ends.
- Road records come from a **journal** in `RoadNetwork`. While a history command runs, the five
  primitives note each road ID they touch and its record before the first change. After the
  command, every touched ID gives its old record (if it was active before) to `before` and its
  current record (if it is active now) to `after`. Roads added and tombstoned in the same
  command drop out. A road whose position is unchanged but whose class changed is kept as a
  **class change** record, so undo can change it back in place. The journal covers alleys the
  command's lot re-cut added and straight joins anywhere. Power records come the same way from
  the four `Power` primitives. Records are sorted by kind, then ID, so steps are deterministic.

Rejected:
- **Inverse operations by ID.** Undo and redo create new IDs (ADR 0013), straight joins
  tombstone one road and extend another, and zoning alleys split roads after the edit. Stored
  IDs go stale after the first undo.
- **Whole-world or whole-network snapshots.** Large, and a world snapshot would bring back
  buildings, residents and money.
- **Re-running the command log without the undone command.** Loaded and shared cities have no
  log (ADR 0034), and replay also rewinds everything else.

### How undo and redo apply a step

Undo applies a step backwards (remove `after`, restore `before`); redo applies it forwards.
Both are all or nothing:

1. **Match.** Each road record to remove must be present: active roads of its class that
   together run exactly along its shape, end to end (within 1 cm, as in 0040). Several pieces
   are allowed, because zoning may have cut alleys into it since. A class change record must
   find its road at the old position. A plant record must match an active plant of that kind
   and centre; a line record an active line with those ends, either way round.
2. **Place.** Each road record to restore uses the node at its exact end positions (kept unused
   nodes included), else splits the road through that point, else a new node; roads it crosses
   are split as in `snap::plan`. There is no snapping and no 15 m pull. A road that would
   overlap an active road lengthwise fails. A plant is restored at its stored footprint and
   must pass the placement checks (`spot_fits`: no plant overlap, clear of roads, coal beside
   a road); a line at its stored ends and must not duplicate a standing line.
3. **Money.** Undo refunds `cost` exactly. Redo charges it again and is refused if the city
   cannot pay, like any build (ADR 0036). Undo is never refused for money.
4. **Apply** with the existing primitives (new IDs for everything restored), then run the usual
   tail of each edit: for roads `join_straight_roads`, `sync_roads`, `remove_cars_using` on
   every touched road and `update_lots` over their bounds; for a restored plant, unzoning the
   cells under it and removing buildings it covers; for a restored line, removing buildings in
   its corridor, exactly as placing them does. The journal records this run too.
5. **Move the step.** Undo pushes its *observed* change, reversed, onto the redo list; redo
   pushes its observed change onto the undo list. Each step therefore reverses what the
   previous one really did, even when a straight join or alley cut made it differ.

If any check fails, the command is refused, both lists stay as they are, and the frame says
why. The step stays; older steps stay blocked until the player clears the obstacle.

With linear history, a step can only be undone after every later step is undone, so at undo
time the map is the post-edit map plus whatever zoning, demolition and growth did since.
Matching allows for zoning alleys; placement checks catch the rest.

### Grouping and continuous curves

- **One history command is one step**, except continuous curve chains. An upgrade drag is one
  `BuildRoad` (0048) and a grid one `BuildGrid` (0049), so each is one step.
- **A continuous curve chain is one step.** `BuildCurve` gains `chain: bool`, set by
  `World::draft_command` when the draft's start carries bit 8, that is, for every piece after
  the first. History keeps one saved flag, `chain_open`:
  - a successful `BuildCurve` without `chain` makes a new step and sets `chain_open`;
  - a successful `BuildCurve` with `chain` joins the newest undo step (adding its records and
    cost) when `chain_open` is set, else makes a new step and sets it;
  - every other successful command, `Undo` and `Redo` included, clears `chain_open`. A refused
    piece does not, so the player can retry it.
- Ending or cancelling a chain in the tool sends nothing. The tool's next chain starts with an
  unflagged first piece, which makes a new step, and any other edit closes the chain anyway.
- **Undo mid-chain** removes every piece drawn so far as one step and closes the chain; the
  tool drops its chain state on undo or redo. A stale chained piece would make its own step,
  and 0046's source check refuses it once the previous piece is gone. Redo restores the whole
  chain and does not reopen it.
- Commands outside the scope, and refused commands, make no step.

### What clears the redo list

Any successful placeable edit that makes or joins a step clears the redo list. Zoning,
building demolition and spawning do not; redo after them still goes through the checks above.

### Depth and save size

- A road record is about 20 to 30 bytes in postcard, a plant record about 20 and a line record
  about 16. A typical road build is 3 to 8 records (about 200 bytes); a plant or line step is
  1 record; a full 20×20 grid is up to about 900 records (about 25 KB). Plants and lines do not
  change the estimate.
- The undo and redo lists together hold at most **100 steps and 10,000 records**. Past either,
  the oldest undo steps are dropped, then the farthest redo steps (an undo can record more
  than the step it took back, when an alley split a road since); the newest step goes only if
  it alone breaks the caps, so every saved history loads. That caps history at
  roughly 300 KB in the save, far below the shared-link limit (ADR 0035).

### Effects outside history

- **Buildings and lots.** Buildings an edit removed stay gone. Restoring a road, plant or line
  may remove buildings that grew since, exactly as placing it by hand would.
- **Zones.** Never changed by undo itself. Undoing a plant leaves its cells unzoned, as
  bulldozing it does; redoing it unzones them again, as placing it does.
- **Agents.** Removed or reshaped roads get the same treatment as a bulldoze or split today
  (`remove_cars_using`); class changes keep routes. When undo or redo lets straight roads merge
  (0040), cars on the merged roads may be removed, as with road upgrades today. The owner
  accepted this for now.
- **Power.** Grids and who is powered are derived, so they recompute after undo and redo as
  after any edit. Undoing a road a plant or line relies on splits the grid; nothing else is
  removed. Roads can already be built over plants; undo adds no check for that.
- **Upkeep** follows the roads, as for any edit (`refresh_upkeep`).

### Commands, logs and saves

- `Command::Undo` and `Command::Redo`, no fields, logged as `<tick> undo` and `<tick> redo`.
  `BuildCurve`'s flag is logged as a trailing word: `curve ax ay cx cy bx by main chain`.
  Older logs read the same (no `chain` means false), so `LOG_VERSION` stays 1, as when 0036
  added `map` and 0049 added `roadgrid`. Replays therefore group chains exactly.
- History, with `chain_open`, is a `World` field in a new `world/history` module, saved with
  the city. Shared files and links carry it, since they are the save bytes (ADR 0035). The save
  version becomes 5; older saves and links are refused, with no reader for version 4. Loading
  validates history: finite coordinates, known kinds and classes, and the caps.
- **History is not mixed into `state_hash`.** It changes nothing until `undo` or `redo`, so
  existing replay fixtures keep their `expect` lines, which also shows that recording is
  passive. New fixtures undo and redo builds, upgrades, a grid, a curve chain, a plant, a line
  and bulldozes on both sides of the halfway save and load, which proves history is saved.

### Preview and interface contract

- Frames carry `history`: `{ undo: kind | null, redo: kind | null, redoCost: number | null,
  redoAffordable: boolean, undoRefund: number | null }`, enough to enable and label the
  buttons ("Undo: place coal plant") and show an unaffordable redo in red. Sent when history or
  the balance changes.
- `previewUndo` / `previewRedo` answer with `remove` and `restore`, each `[n, x0, y0, …]` per
  record: a road's centreline (curves sampled as in the road snapshot), a plant's closed
  outline or a line's two ends; plus `ok`. Read-only.
- `undo` and `redo` messages carry an `id` and are answered `{ ok }`. Ctrl+Z (Cmd+Z) and
  Ctrl+Shift+Z (Cmd+Shift+Z) work in any tool. Layout and styling need no ADR.

## Amendment (2026-11-07): water, fire and costs

Water (ADR 0052), fire (ADR 0053) and children (ADR 0054) landed after this ADR was accepted.
Where this section and the text above disagree, this section wins.

- **Scope.** Pumps and towers (`PlaceWater`), fire stations (`PlaceStation`) and a `DeleteRoad`
  that removed one join the history, as the scope above foresaw. A pump, tower or station
  record is `{kind, centre, front}`, the placed footprint, like a plant. Clearing ruins stays
  out, like building demolition. `DeleteRoad` now removes a station, else a plant, else a line,
  else a pump or tower, else a building, else ruins, else a road.
- **Money.** Plants, pumps, towers and stations now cost money (ADR 0052, 0053), so their steps
  carry a cost like roads: `cost` is what the command took from the balance. Undo refunds it and
  redo charges it again, refused when the city cannot pay.
- **Restoring pieces.** A pump, tower or station is restored at its stored footprint and must
  pass the same placement check as placing it by hand (`spot_fits`, beside a road). A restored
  tower starts empty. A restored station gets new trucks; removing one retires its trucks, as
  bulldozing does.
- **How changes are found.** Instead of hooking each primitive, a history command copies the
  per-road ends, control point, class and active flag, and the active flags of plants, lines,
  pumps, towers and stations, before it runs, and compares them after. Node positions never
  change and IDs are append-only, so this finds the same records as the journal, cannot miss a
  primitive, and costs one pass over the roads per player edit.
- **Saves.** The save version becomes 8 (version 7 added children), not 5.
- **Already built.** `SetRoadClass` (0050) and `BuildGrid` (0049) are implemented as described.

## Consequences

- One mechanism covers every placeable present and planned. A new road command needs no
  history code beyond naming its kind if it uses the five primitives; a new placeable kind adds
  a record kind, its match and placement checks, and a save version bump.
- The risky new sim pieces are road matching (a record covered by several pieces) and exact
  placement without snapping. Both need focused tests: a straight join undone and redone, a
  bulldozed side street restored, a road split by a zoning alley undone, a grid and a curve
  chain each undone in one step, a plant undone and redone over regrown buildings.
- `BuildCurve` gains a field, so every place that builds or matches it changes, and an older
  build cannot read a log with `chain`.
- Undo can be refused when the map changed outside history in a way it cannot match or fit,
  for example an alley cut lengthwise where a bulldozed road stood. This should be rare.
- An alley cut by zoning into a road that undo later removes stays behind as a dead end.
- Undo refunds money, which ADR 0036's free bulldozing never did. Players can "rent" a road
  for its upkeep by building and later undoing it while it is within reach of undo.
- Save version 5 makes every existing city and shared link stop opening. Other branches that
  bump the save version must coordinate.
- Ctrl+Z after demolishing a building or painting zones undoes the last placeable edit, not
  that action; the button label names the step it undoes.
