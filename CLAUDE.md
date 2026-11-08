# Project rules for agents

Architecture decisions and their reasons live in `docs/adrs/`. Read the index before changing
architecture. If a change contradicts an ADR, say so and write a superseding ADR.
What to build next and why lives in `docs/ROADMAP.md`. Work on the current milestone unless told otherwise.

## Architecture
- The sim (`crates/sim`) is the source of truth. The frontend never computes game state; it sends
  commands (`web/src/protocol.ts` → worker → `World::apply`) and draws snapshots.
- The sim must stay deterministic: fixed `dt`, seeded `Rng`, no `HashMap` iteration order in logic
  that affects state, no wall clock, no floats from JS feeding state except through `Command`.
  `World::state_hash` exists to check this.
- Storage is struct-of-arrays (`Vec` per field). Keep it that way for anything with 10k+ entries.
- Ids (`NodeId`, `LaneId`, `AgentId`) are indices and append-only.
- `crates/sim-wasm` only converts types. Logic goes in `sim`.
- Hand-written code files, tests and scripts included, stay at or under 500 physical lines.
  Split a growing file by responsibility into a module folder (`foo/mod.rs` plus private child
  modules; integration tests as `tests/foo/main.rs`), not into numbered chunks.
  `scripts/line-limit.mjs` enforces it and skips generated WASM bindings, dependencies and docs.

## Snapshot formats
- Agents: `[x, y, heading, speed_ratio, kind]` per agent (`SNAPSHOT_STRIDE`, `AGENT_STRIDE`),
  speed_ratio < 0 = hidden, kind 0 = car, 1 = walker on the sidewalk (ADR 0026), 2 = fire
  truck (ADR 0053; not a citizen, so inspecting one answers null). Frames go out at most
  ~60 a real second; the page returns drawn agent buffers with `spareAgents` for the worker
  to refill, so a frame's agents may view a larger buffer (ADR 0003, amended).
- Roads: `[ax, ay, bx, by, class]` per active rendered segment, class 0 = street, 1 = main road,
  2 = alley; curves emit multiple segments. Packed in road ID order, not indexed by road ID
  (ADR 0015, 0017).
- Bulldoze preview: `[x, y]` per centreline sample of the selected road, or a closed outline
  for a fire station, plant, pump, tower, building or ruins (ADR 0045, 0052, 0053); empty =
  no target. Stations, plants, lines, pumps and towers, buildings, then ruins win over roads.
  Read-only. Road/lane IDs are never reused after deletion (ADR 0013). Lane counts include
  tombstone slots.
- Road class clicks (ADR 0050): `setRoadClass` gives the road nearest `[x, y]` in bulldoze
  reach a class in place, as drawing over it would (ADR 0048); command logs write it as
  `class x y main`. `previewRoadClass` answers `roadClassPreview`: `segment` is `[x, y]` per
  centreline sample of the road that would change, empty = nothing would change (no road, or
  already that class), plus `cost` (null when empty), `affordable` and the echoed revision.
- Signals: `[x, y, heading, state]` per signalised approach (`SIGNAL_STRIDE`), state 0 = red,
  1 = green, 2 = protected-left green.
- Road drafts: `previewRoad` and `buildRoad` send raw `[x, y, snap]` points (start, a curve's
  control, end); snap bits 1 = angle, 2 = guide lines, 4 = length. For a continuous curve,
  bit 8 on the start means the remaining records are end and the previous sim-guided control;
  the sim derives the new control, or returns no points if refused (ADR 0046). The sim guides
  them (ADR 0038) and builds with the guided coordinates, so command logs record final positions.
  `previewRoad`, `buildRoad`, `previewGrid` and `buildGrid` carry an optional `reach`: ten
  screen pixels in ground metres at the cursor; guide lines, junctions and angle steps capture
  within it, never less than the sim's 6 m / 15 m / angle tolerance (absent = those). The worker
  appends it to the raw points as one trailing float for WASM (ADR 0038, amended).
  The preview answer has `points` (`[x, y]` per guided point), `guides` (`[x0, y0, x1, y1]` per
  guide line), `length`, `angle` (NaN = none), `angles` (`[x, y, rx, ry, dx, dy]` per junction
  the road joins: the point, then unit directions along the road measured from and the new
  road; the end's only when planned) and `stops`: `[x, y, join]` per sample of the road
  it would build, join -1 = interior curve sample (no marker), 0 = new end, 1 = existing node,
  2 = splits a road; empty = refused or not planned. One point previews where a road would
  start. Echoes the request revision so outdated previews can be discarded. Also carries `cost`
  (dollars the guided road would charge, null when nothing is planned) and `affordable`
  (false = the build would be refused for money; ADR 0036). Changes nothing. The request's
  `class` matters: roads the draft follows take that class when built, so a draft over roads
  of the same class plans nothing (ADR 0048). `reclass` lists those roads, `[n, x0, y0, …]`
  per road whose class would change (its centreline samples, as the Upgrade click preview);
  empty = none. The road tool names them in its hint and highlights them at the new width.
  `buildResult` carries the guided `end`, null if refused, and the guided `control` for a
  successful curve when the request sets `chain`.
- Grid drafts (ADR 0049): `previewGrid`/`buildGrid` send the first edge's two raw
  `[x, y, snap]` records, `cell` metres and an optional area `cursor` `[x, y]`; the sim guides
  the edge and rounds the area to whole blocks. `gridPreview` carries the road draft's
  `points`, `guides`, `length`, `angle`, `angles`, plus signed `cols`/`rows`, `roads`
  (`[ax, ay, a_join, bx, by, b_join]` per new leg, joins as `stops`), `dropped`
  (`[ax, ay, bx, by]` per left-out lattice edge), `cost` and `affordable`. One `buildGrid` is
  one all-or-nothing `BuildGrid` command (`roadgrid` in command logs); it answers `buildResult`
  with `end` null.
- Zones: one byte per zone cell, row-major, plus layout `[origin_x, origin_y, cell_size, cols,
  rows]`; byte 0 = none, 1/2 = residential low/high, 3/4 commercial, 5/6 industrial (ADR 0014).
  Sent whole when the zone grid's version changes.
- Pollution: four bytes per pollution cell, row-major, `[air, ground, noise, land_value]`,
  0 = none (land value: worthless) to 255 = full, plus a layout like the zones'; cells are
  4×4 zone cells sharing the zone origin (ADR 0037). Sent whole when its version changes,
  at most once a real second while running; start, edits and pause flush immediately.
- Lots: `[n, x0, y0, …]` per lot, its outline's point count then its points, sent whole when
  the lots version changes (ADR 0018). Block preview: `[x, y]` per outline sample of the block
  under the cursor, closed; empty = no block. Read-only.
- Buildings: `[zone, height, roof, front, n, x0, y0, …]` per active building in ID order, not
  indexed by ID. Height is metres; n is the counter-clockwise footprint point count, with no
  repeated closing point. Roof is a stable palette index (0–3 for industry, 0 otherwise; ADR 0019).
  Front is the footprint edge (point `front` to the next) facing the lot's road, derived on
  write, not stored or hashed (ADR 0027).
  Zone is the building's original use. The worker sends a parallel `ids: Uint32Array`, one
  stable building ID per packed record in the same message (ADR 0029). Sent whole when the buildings version changes,
  including growth, closure removals (ADR 0023)
  and removals from paused road edits. Growth snapshots are capped at four
  per real second; start, edits and pause flush immediately.
- Power (ADR 0041, 0042): plants `[kind, x, y, half_size, front_x, front_y, terminal_x,
  terminal_y, output, grid]` per plant (`PLANT_STRIDE`), kind 0 = coal, 1 = solar, 2 = wind;
  the square footprint is turned to face the unit front (toward its road for coal), and lines
  join at the terminal; lines `[ax, ay, bx, by]` per line; buildings
  `[x, y, height, grid, powered]` per standing building (`POWER_BUILDING_STRIDE`), grid -1 =
  none. Grid numbers are stable within one message only. Sent whole when plants, lines, grids
  or who is powered change; growth is capped like buildings. Plant preview: closed `[x, y]`
  footprint where the plant would stand (coal snapped beside a road) plus `ok`. Line preview: `[ax, ay, a_join, bx, by, b_join]`, join 1 = meets a line,
  plant or road; empty = refused. Bulldoze removes a fire station, else a plant, else a line,
  else a pump or tower, else a building, else ruins, else a road. Placing a plant is refused
  when the city cannot pay (ADR 0052).
- Water (ADR 0052): networks follow roads, derived with power's road grids, no pipe tool.
  `placeWater`/`previewWater` take `kind` 0 = pump, 1 = tower and a point; the piece snaps
  beside the nearest road facing it, refused off roads, over other pieces or when unaffordable.
  `water` sends pieces `[kind, x, y, half_size, front_x, front_y, output, level, network]` per
  pump or tower (`WATER_PIECE_STRIDE`; output 0 for towers, level 0 for pumps) and buildings
  `[x, y, height, network, watered]` per standing dense building (`WATER_BUILDING_STRIDE`),
  network -1 = none, stable within one message only. Sent whole when pieces, networks, tower
  levels or who has water change; growth is capped like buildings. Water preview answers
  `waterPreview` with a closed `[x, y]` footprint plus `ok`, like the plant preview. Frames
  carry `water` stats: `supply`, `used`, `demand`, `stored`, `capacity`, `dry`, `waiting`.
- Fire (ADR 0053): `fire` carries stations `[x, y, half_size, front_x, front_y]`
  (`STATION_STRIDE`), a square turned to face its road like coal; fires `[x, y, height, truck]`
  per burning building (`FIRE_STRIDE`), oldest first, truck 1 = one is on the way; ruins
  `[n, x0, y0, …]` per ruined lot like lots; `coverRoads` `[ax, ay, bx, by]` per rendered
  segment of a road a station reaches; `coverBuildings` `[x, y]` per covered building. Sent
  whole when stations, fires, ruins, the setting, roads, lots or buildings change; growth is
  capped like buildings. Station preview: closed `[x, y]` footprint plus `ok`. `setFires`
  turns the city's fires on or off (`fires on|off` in command logs; `station x y` places one).
  Frames carry `stats.fire` (`FireStats`). Flames, smoke and rubble are display only.
- Undo and redo (ADR 0051): the sim keeps the history and saves it with the city. `undo` and
  `redo` answer `historyResult` (`ok` false = refused); command logs write them as `undo` and
  `redo`, and continuous curves after the first as `curve … chain`. Frames carry `history`: the
  step kinds undo and redo would take (null = none), `undoRefund`, `redoCost` and
  `redoAffordable`. `previewHistory` answers `historyPreview`: `remove` and `restore`,
  `[n, x0, y0, …]` per road centreline, closed plant/pump/tower/station outline or line's two
  ends, plus `ok`; read-only, echoing the revision.
- Citizen inspection: `inspect` names a car or walker by its agent snapshot index; the worker maps it once
  to a citizen and answers `citizen` for that one citizen only, at once and after every frame
  and edit while selected (ADR 0022). Read-only, not hashed; names are derived from the seed and
  IDs, never stored or drawn from a gameplay RNG. Places are absent once their building is gone.
  The answer carries `age` (years; a child until 18), `child` (lives at home), `grewUp` (an
  adult who came of age in town) and `wantsHome` (still with their family, looking for a home
  of their own; ADR 0054).
- Population: frame stats carry `population` (children included) and `children`;
  `unemployed` and its percentage count adults only (ADR 0054).
- Building inspection: `inspectBuilding` selects a stable building ID from the geometry message's
  parallel `ids` array. Replaces citizen inspection and vice versa. The worker answers `building`
  at once and after every frame/edit while selected, echoing the selection counter; null after
  demolition. Capacity and occupancy are sim-owned residency/employment counts, not attendance. `watered`
  is false for a dry dense building (ADR 0052).
  Read-only, not hashed (ADR 0029).
- Shared cities: a `.city` file is the ADR 0033 save bytes unchanged; a link is `#city=` plus
  base64url(raw deflate(save bytes)) in the URL fragment, refused above 1,000,000 characters
  (`web/src/city-share.ts`, ADR 0035). Opening one replaces the local city as an independent copy.
- Money: the sim owns the balance, build costs, taxes, a flat grant and upkeep (ADR 0036);
  frames carry `balance`, `taxes`, `grant`, `upkeep` (roads) and `utilities` (plants, pumps
  and towers; ADR 0052), in dollars and rates per clock hour. A fresh game city is an
  empty map (`World::empty`); the generated grid is for tests and benchmarks.
- Command logs: versioned text owned by `sim::replay` (ADR 0034); the third line is `map` (empty
  map, as the game starts) or `grid` (generated streets); fixtures live in
  `crates/sim/tests/replays/*.cmdlog`. Refresh `expect` lines only for intended gameplay changes
  with `BLESS_REPLAYS=1 cargo test -p sim --test replay`.
Change both sides (Rust writer, `protocol.ts` doc, `renderer.ts` reader) together.

## Checks before finishing a change
- `scripts/check.sh` (same as CI): gitleaks over full history, the 500-line file limit, rustfmt, clippy `-D warnings` (host + wasm32), `cargo test -p sim`,
  WASM worker and frontend geometry tests (Node 24+), Biome lint/format, `tsc`.
  `scripts/check.sh --fix` applies formatting and safe fixes first.
- The repo is public. `.githooks/pre-commit` runs gitleaks on staged changes; enable it per clone
  with `git config core.hooksPath .githooks`. Never bypass it with `--no-verify`.
- `clippy.toml` bans `HashMap`/`HashSet` and `Instant::now`/`SystemTime::now` to protect
  determinism. Use `BTreeMap`/`BTreeSet` or `Vec` storage; only `#[allow]` with a comment where
  state is unaffected (benches).
- For perf-sensitive sim changes, rerun `traffic_bench` and compare ms/tick, at the default size
  (ADR 0011) and with `--scale la` (ADR 0012).

## Licensing
Only MIT / Apache-2.0 / BSD / ISC dependencies. GPL/AGPL projects (Egregoria, Citybound, movsim)
are reading material only; do not copy code from them.
