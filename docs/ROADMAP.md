# Roadmap

**Core fantasy: grow a village into a city yourself.** You lay the roads and paint the zones,
and the player is always able to build the whole city by hand.

**Novel feature: councilors.** An optional helper, not the core of the game. Draw a district,
give a councilor a brief ("dense housing", "industry hub"), and they lay roads and zones inside
it while you work elsewhere. You can override anything they do, or never hire one at all.

**Arc: village to city, in eras.** Population unlocks each era. Old buildings stay until demand
redevelops them, so the old core and the new streets sit side by side. Version one has two eras,
village and town. More come once that transition works.

**Look: stylized low-poly.** Flat-shaded, procedurally generated meshes, cheap to draw at scale.

**Citizens you can meet.** Click any car or person to see who they are, where they live and work,
and where they're going, and have the camera follow them.

**Sessions:** quick to start for a 15-minute visit, deep enough for a long evening.

**Platform:** desktop only (mouse and keyboard), in the browser first, then Steam. The map is flat
until much later. **1.0 is the Steam release.**

**First share: a playable toy.** A link friends can open to draw roads and zones and watch a city
grow. It has no economy and no way to fail.

Milestones run in order. Each one ends with something you can see in the browser. "Done when"
lines are the acceptance checks. Items marked **ADR** lock in a contract and need an ADR before
code lands (see [adrs/AGENTS.md](adrs/AGENTS.md)).

## Done

- **M0 – Traffic sandbox.** Road graph, A* pathfinding, lane-queue traffic, 50k agents in the
  browser, road tool, camera, determinism hash, CI checks.

## M1 – Roads that work

A growing city makes traffic. The traffic model has to hold up before anything grows on it.

- Intersection rules so circular waits stop gridlocking the network. **ADR** (replaces the known
  gap in ADR 0006).
- Snapping: new roads join existing nodes and split segments they cross.
- Curved roads (Bézier), behind the existing `lane_point` / `lane_heading` queries.
- Road deletion (bulldoze): implemented with tombstones ([ADR 0013](adrs/0013-road-deletion-tombstones.md)).
- Road meshes in place of lines: asphalt ribbons with simple intersection pads.

Done when: 50k agents drive for a sim-hour on a hand-drawn network with near-zero gridlock
despawns, and the road tool feels good with snapping, curves and bulldozing.

## M2 – A city that grows (playable toy)

- Zoning brush: residential, commercial and industrial, each with a low- and high-density
  brush (houses vs. apartments, corner shops vs. stores). You choose density; it doesn't creep
  up on its own. Zones live on a fixed grid ([ADR 0014](adrs/0014-zone-grid.md)).
- Block-filling lots: zoning paints whole blocks, and the sim cuts lots to fill the space
  between roads, including odd shapes left by curves. Where a block is too deep to reach from
  its edges, the sim adds alleys and small connector streets to reach interior lots. These
  are ordinary roads in the road graph, so cars use them and the player can bulldoze them.
  ([ADR 0016](adrs/0016-blocks-lots-and-alleys.md)). Lots are shapes cut along the roads,
  wedges on curves ([ADR 0018](adrs/0018-lots-cut-along-roads.md)).
- Buildings appear on zoned lots over time, as procedural low-poly boxes by zone and density.
- Repainting or unzoning occupied land doesn't bulldoze it: the old buildings empty out over
  time and the new zone replaces them. Implemented ([ADR 0023](adrs/0023-gradual-closure-of-rezoned-buildings.md)).
- Citizens get a home and a workplace. Trips run home → work → shop → home on a daily
  schedule, replacing random node-to-node trips ([ADR 0021](adrs/0021-citizens-households-and-trips.md)).
  Households, homes, limited jobs, the resident population counter and unemployment
  statistics are implemented.
- Walking trips: most adults own a car, some don't. Short trips are walked, and adults
  without a car only take jobs and shops within walking reach. Walkers show as small figures
  on the sidewalks, wait at red lights and can be clicked in the citizen inspector. The HUD
  shows the share of trips made on foot
  ([ADR 0026](adrs/0026-walking-trips-and-household-cars.md)).
- Route choice that feels human:
  - Road classes: trips use local streets to reach a main road, ride it, then leave it near the
    destination. Routing searches the main-road layer for the middle of long trips, which is
    also faster.
  - Some randomness: each driver adds a small seeded random change to route costs, so cars
    don't all pick the same "best" route and traffic spreads out. Implemented; the
    per-queued-car delay stays as it was ([ADR 0024](adrs/0024-per-driver-route-noise.md)).
- Citizen inspector: click a car to see the citizen's name, home, job and destination; a follow
  camera rides along. Implemented as a read-only query for the selected citizen
  ([ADR 0022](adrs/0022-citizen-inspection-query.md)).
- Building inspector: click a building to see resident capacity and residents, or total,
  filled and unfilled jobs. Counts describe residency/employment, not physical attendance.
  Shares the citizen panel and refreshes only the selected building; implemented
  ([ADR 0029](adrs/0029-building-inspection-query.md)).
- Demand: a simple R/C/I balance decides what gets built next. Implemented: zoned lots build
  only while their use has demand, shown as R/C/I bars in the HUD
  ([ADR 0025](adrs/0025-rci-demand-gates-growth.md)).
- Day clock and an on-screen time and speed indicator.
- Minimal UI: tool bar (road, zone, bulldoze), population counter.
- Hosted build (e.g. GitHub Pages) so the city is a link.

Done when: someone who has never seen the game opens the link, builds a town of a few
thousand people without instructions, follows a citizen home, and wants to keep going.

## M3 – Looks and feel

The point where the toy starts to look like a place.

- Low-poly art pass: building variety by zone and density, trees, terrain color, water. Build
  the building kit so it can carry an era style (village cottages vs. town blocks) later.
  Low-density commerce now grows small, shallow shops, mostly one storey, with individual
  doors and awnings, varied roof forms, planted rear space and occasional attached pairs;
  high-density commerce keeps full-lot buildings ([ADR 0031](adrs/0031-small-low-density-shops.md)).
- Lot scenery (implemented): hide zoning colors outside zoning
  mode. Developed lots get display-only outdoor details in the existing low-poly style:
  green, varied house gardens and paths, planted shop entrances and paved factory service
  yards, with modest parking. These details do not affect gameplay. Vacant zoned land shows
  natural ground rather than zoning colors or prepared plots outside zoning mode. Painting
  still clears wild trees and fields, leaving grass before development. Outdoor details
  follow the standing building's use during closure, not the newly painted zone, and go
  away when the building does. Drawn lot outlines appear only in zoning mode. Houses choose
  stable front gardens, lawns or street-connected parking where space allows, with narrow paths
  aligned to their doors. Backyards choose lawn, a furnished patio, vegetable beds or a tree,
  rather than repeating every prop. Residential boundaries mix joined hedges, warm-white picket
  fences and open lots. Factory yards choose stable ground, a metal fence, block wall or no
  boundary, a truck court or single drive to a loading door, and one main use: pallets,
  containers, storage tanks or staff parking. Paths, driveways and aprons stay open,
  and boundaries stop at buildings and roads. Shops keep open frontages with varied paving and
  one kind of frontage prop, or front parking when set well back; small shops add one rear use,
  most often a storage yard with a shed and stock. Boundaries take priority
  over loose props so a tree or planter cannot remove a property side. These choices are visual
  only, not parking rules. Fit details to the available outdoor space rather than changing
  building footprints.
- Lighting and shadows, and a day/night cycle that follows the sim clock (lit windows, headlights).
- Varied vehicles (cars, vans, trucks), and detailed pedestrian figures in place of the M2
  placeholders.
- Ambient sound (implemented): soft engine pass-bys near moving cars and daytime birds.
  No continuous traffic or building-noise loops. Sparse traffic stays quiet, stopped/hidden
  cars are silent, and zooming out only reduces volume rather than collecting more sources.
- Juice: buildings rising on construction, zone paint feedback, smooth tool previews.

Done when: a screenshot or short clip is something you'd post.

## M4 – Save, load, share

- Save and load. The first slice is implemented: one local full-state save with autosave
  and versioning ([ADR 0033](adrs/0033-single-city-state-saves.md)). This slice is pulled
  forward at the player's request; the rest of M4 remains later work. Scenarios (M5) can
  later use the state payload as their start state, with goals kept separate.
- Share a city as a link or file. Opening one forks it: the friend keeps building their own copy
  from where you left off.
- Replay-based regression tests built from saved command logs.

Done when: a city survives a reload, and a friend can open a city you sent them and keep
building it.

### First slice: one city with autosave — implemented

Simulation, worker, storage and UI checks pass, including identical continued evolution after
restoring a developed town. `scripts/check.sh` and the production build pass. The browser
save/reload/new-city smoke test remains unverified: terminal-browser requires Ghostty to be
running, and it is unavailable in this session. Sharing and backups remain later work.

**Player flow**

- Opening the game reads the local slot before creating a fresh city. A saved city resumes
  paused at its saved tick. A missing save starts a normal fresh city. A failed read or
  invalid save shows an error without silently clearing or overwriting the slot.
- Save city saves immediately. Autosave requests a save every 60 real seconds when the
  simulation has changed, including paused edits; it does not depend on simulation speed.
  Ordinary ticks and successful gameplay edits make the city unsaved; previews, inspection,
  camera motion and speed controls do not. Do not rely on asynchronous work at tab close.
- Show saving progress, the last successfully saved time, unsaved changes and clear errors.
  A commit acknowledges only the state it captured: changes made while saving remain unsaved.
  Failed saves retain the prior committed city and can be retried.
- New city asks before discarding progress and replacing the saved city. Cancel changes
  nothing. Confirm creates the normal fresh city and immediately attempts to save it, with
  errors reported honestly and no late save from the previous city able to overwrite it.
- Explain that storage belongs to this browser and site, and clearing browser data can lose
  the city. No save history, file backups, sharing, accounts, camera persistence or conversion
  of incompatible older saves is included.

**Delivery record**

1. **Sim persistence and tests — complete.** Add a persistence module under `crates/sim/src/world/` with
   byte-oriented save/load methods. Inventory every authoritative field and distinguish it
   from reconstructible scratch state. Write failing round-trip and continued-evolution tests
   before adding the codec. Keep IDs, event ordering and RNG streams intact; reject malformed
   or incompatible data before replacing a world. Follow the existing module structure and
   500-line limit rather than concentrating serialization in `world/mod.rs`.
2. **WASM and worker lifecycle — complete.** Expose thin byte conversions in `crates/sim-wasm`; extend
   `web/src/protocol.ts` for persistence requests and status. In the worker, serialize saves
   between complete ticks and coordinate startup, manual saves, autosaves and confirmed city
   replacement. Reset version caches, selections and clock backlog on replacement; publish
   all geometry even if the new world's version numbers equal the old world's. Keep one tick
   loop, release replaced WASM worlds and prevent stale requests from changing the new city.
3. **Browser storage and controls — complete.** Add a small persistence module beside `sim.worker.ts`
   using IndexedDB, not a new service or backend. Store payload and timestamp atomically in
   one slot. Add Save city, New city, confirmation and accessible status/error text in
   `web/index.html`; put UI coordination in a focused module rather than growing `main.ts`
   past the line limit. Clear interpolation, previews and inspector state on replacement.
4. **Integration checks and documentation — automated checks complete; browser smoke test
   blocked as noted above.** Extend the existing Rust, WASM-worker and frontend
   test harnesses. Update the root README only once saving is implemented; mark this plan's
   completed steps and ADR 0033's implementation status accurately. Run `scripts/check.sh`,
   the production build and a browser save/reload/new-city smoke test.

**Acceptance checks**

- A developed town with moving cars and walkers, pending growth, edited roads and occupied
  buildings closing after rezoning survives save/load. Compare state hashes, statistics,
  snapshots and inspection counts immediately, then compare subsequent ticks and gameplay
  commands on the same build and target. Exercise cached indexes both before and after load.
- Empty and paused worlds round-trip. Encoding a save changes neither the live world nor
  its future evolution. Truncated, corrupt and unsupported saves fail safely.
- Reload restores the stored city paused; a missing slot starts fresh. Read failures and
  invalid saves cannot be overwritten by autosave. Failed and aborted writes preserve the
  previous committed payload and timestamp, and the UI does not report success prematurely.
  Writes from an outdated tab are rejected atomically; confirmed New city can replace the
  slot. Save retries reopen unexpectedly closed database connections.
- The autosave timer skips unchanged worlds, includes paused edits, and does not mark later
  edits saved when an earlier snapshot commits. Manual saving works without waiting for it.
- Canceling New city retains the current city and save. Confirming cannot be undone by an
  in-flight old save; the new city saves immediately, or clearly reports a storage failure.
  Repeated replacement leaves one clock loop and no stale geometry or inspector selection.

## M5 – A game loop

Goals and consequences, kept light so they don't crowd out building.

- Money: build costs, taxes, upkeep.
- Services with coverage areas: power, water, roads maintenance, then fire, police, health and
  schools.
- Citizen happiness from services, traffic and pollution, feeding back into demand and growth.
  The citizen inspector shows each person's mood and complaints.
- Data overlays: traffic, coverage, land value, happiness.
- Two modes:
  - **Sandbox:** never fails. Problems slow growth or make people move away, but there is no game
    over.
  - **Scenarios:** a set start and goal ("reach 5,000 with no gridlock", "rescue a bankrupt
    town") that you can fail. Built on M4 save files.

Done when: there is a reason to plan and a way to struggle in scenarios, and sandbox still feels
relaxed.

## M6 – Eras and councilors

The village becomes a town, with councilors as an optional helper for players who want one.

- **Eras.** Village (dirt roads, cottages, small shops) and town (paved streets, shopfronts,
  apartments). Reaching a population threshold unlocks the town era. **ADR** for how eras attach
  to roads, buildings and unlocks.
- **Era zones.** Farms are a village-era zone: fields and barns around the edge of town that
  supply early jobs. The town era unlocks offices (clean jobs, no pollution) and mixed-use
  (shops below, flats above) for main streets.
- **Gradual redevelopment.** Village buildings stay until demand replaces them with town-era
  ones. Mark buildings or areas as heritage to protect them.
- **Era-tied transit.** The town era brings buses and trams, with simple stops and lines.
- **Councilors (optional).** Each era after the village adds one councilor slot. Draw a
  district, give the councilor a brief (dense housing, shops, industry hub, green), and they lay
  roads and zones inside it over time. Their plans show as ghosts as they build; you can
  override or bulldoze anything. **ADR** for the councilor planner: it runs inside the sim and
  issues ordinary commands, so it stays deterministic and replays from a save.
- Councilor UI: district outline, brief picker, progress, and a way to recall them.

Done when: a player can take a village to a town entirely by hand, and a player who hires a
councilor for a new district is happy with what they build.

## M7 – Scale and Steam (1.0)

- Reach the scale target in ADR 0012: a Los Angeles-sized city at rush hour (about 250k cars
  driving) at 10x speed, measured with `traffic_bench --scale la`. Culled or delta snapshots,
  shader-side vehicle interpolation, wasm threads if a profile calls for them (ADR 0002).
- Multi-lane roads and lane changes, highways and interchanges.
- Electron or Tauri wrapper, settings menu, keybinding options.
- Steam page, achievements, workshop (only if mods are in scope). **ADR** for mod support.

Done when: the game is on Steam.

## Parking lot

Ideas worth keeping that aren't scheduled:

- A third era (city: towers, metro, highways) and more councilor slots, once village → town works.
- Councilor personalities (planner, sprawler, green, car-first) and councilors who propose plans
  for approval.
- Procedurally generated terrain from a seed (hills, rivers, coast), then terrain editing.
- Shared-seed scenarios with scores to compare with friends.
- A feed of citizen life events (moving in, changing jobs, complaints).
- Districts and policies, disasters, multiplayer (determinism keeps lockstep open), a photo mode.

Out of scope for now: touch, tablet and phone controls.
