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
  speed_ratio < 0 = hidden, kind 0 = car, 1 = walker on the sidewalk (ADR 0026).
- Roads: `[ax, ay, bx, by, class]` per active rendered segment, class 0 = street, 1 = main road,
  2 = alley; curves emit multiple segments. Packed in road ID order, not indexed by road ID
  (ADR 0015, 0017).
- Bulldoze preview: `[x, y]` per centreline sample of the selected road; empty = no target.
  Read-only. Road/lane IDs are never reused after deletion (ADR 0013). Lane counts include
  tombstone slots.
- Signals: `[x, y, heading, state]` per signalised approach (`SIGNAL_STRIDE`), state 0 = red,
  1 = green, 2 = protected-left green.
- Road drafts: `previewRoad` and `buildRoad` send raw `[x, y, snap]` points (start, a curve's
  control, end); snap bits 1 = angle, 2 = guide lines, 4 = length. The sim guides them
  (ADR 0037) and builds with the guided coordinates, so command logs record final positions.
  The preview answer has `points` (`[x, y]` per guided point), `guides` (`[x0, y0, x1, y1]` per
  guide line), `length`, `angle` (NaN = none), `angles` (`[x, y, rx, ry, dx, dy]` per junction
  the road joins: the point, then unit directions along the road measured from and the new
  road; the end's only when planned) and `stops`: `[x, y, join]` per sample of the road
  it would build, join -1 = interior curve sample (no marker), 0 = new end, 1 = existing node,
  2 = splits a road; empty = refused or not planned. One point previews where a road would
  start. Echoes the request revision so outdated previews can be discarded. Also carries `cost`
  (dollars the guided road would charge, null when nothing is planned) and `affordable`
  (false = the build would be refused for money; ADR 0036). Changes nothing.
  `buildResult` carries the guided `end`, null if refused.
- Zones: one byte per zone cell, row-major, plus layout `[origin_x, origin_y, cell_size, cols,
  rows]`; byte 0 = none, 1/2 = residential low/high, 3/4 commercial, 5/6 industrial (ADR 0014).
  Sent whole when the zone grid's version changes.
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
- Citizen inspection: `inspect` names a car or walker by its agent snapshot index; the worker maps it once
  to a citizen and answers `citizen` for that one citizen only, at once and after every frame
  and edit while selected (ADR 0022). Read-only, not hashed; names are derived from the seed and
  IDs, never stored or drawn from a gameplay RNG. Places are absent once their building is gone.
- Building inspection: `inspectBuilding` selects a stable building ID from the geometry message's
  parallel `ids` array. Replaces citizen inspection and vice versa. The worker answers `building`
  at once and after every frame/edit while selected, echoing the selection counter; null after
  demolition. Capacity and occupancy are sim-owned residency/employment counts, not attendance.
  Read-only, not hashed (ADR 0029).
- Shared cities: a `.city` file is the ADR 0033 save bytes unchanged; a link is `#city=` plus
  base64url(raw deflate(save bytes)) in the URL fragment, refused above 1,000,000 characters
  (`web/src/city-share.ts`, ADR 0035). Opening one replaces the local city as an independent copy.
- Money: the sim owns the balance, build costs, taxes and upkeep (ADR 0036); frames carry
  `balance`, `taxes` and `upkeep` (dollars, rates per clock hour). A fresh game city is an
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
