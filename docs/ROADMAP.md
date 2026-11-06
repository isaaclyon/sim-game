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

- Low-density houses now choose a seeded 8–14 m setback from the road centreline, even
  on roomy lots, so front yards vary. Existing buildings keep their footprints.
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
- Juice (implemented): new buildings take 9.6 seconds to build: scaffolding (steel
  standards, timber decks, rails and a brace) goes up, the building climbs inside it in pale,
  unfinished materials with dark windows, then the scaffolding comes down; freshly painted zone cells flash a lighter shade and fade;
  road and bulldoze previews are translucent road-wide ribbons and snap markers glide between
  sim answers. All of it is display only, the sim's buildings are whole from the start, and
  a city's first snapshot after start or load appears at once. Reduced-motion settings skip it.

Done when: a screenshot or short clip is something you'd post.

## M4 – Save, load, share

**Status: complete.** All three items are implemented and the done-when checks pass in a
browser (see "Second slice" below).

- Save and load (implemented): one local full-state save with autosave and versioning
  ([ADR 0033](adrs/0033-single-city-state-saves.md)). Scenarios (M5) can later use the state
  payload as their start state, with goals kept separate.
- Share a city as a link or file (implemented). Opening one forks it: the friend keeps building
  their own copy from where you left off ([ADR 0035](adrs/0035-share-cities-as-forked-copies.md)).
- Replay-based regression tests built from saved command logs (implemented,
  [ADR 0034](adrs/0034-command-log-replay-tests.md)).

Done when: a city survives a reload, and a friend can open a city you sent them and keep
building it.

### Second slice: sharing and replay tests — implemented

- **Sharing.** Download writes the save bytes as a `.city` file; Open file reads one back.
  Copy link puts the compressed save in a `#city=` URL fragment, shown in a field as well as
  copied, and refuses links over 1,000,000 characters. Opening either replaces the local city
  with an independent copy, paused, and saves it at once. The game asks first whenever a city
  could be lost, and only skips the question for a link that starts the game in a browser
  with no saved city.
  Links pasted into a running game also open. Damaged links and files change nothing. The page
  drops the fragment after reading it, so a reload resumes the local copy.
- **Replay tests.** `sim::replay` reads and writes versioned text command logs; fresh cities
  record every command in the WASM wrapper, and development builds download them with the
  Command log button. `crates/sim/tests/replays/` holds a browser-recorded session and a
  scripted one covering every command kind. The test replays each, compares hash, population,
  buildings and trips with its `expect` line, and checks that a save and load halfway through
  ends the same. `BLESS_REPLAYS=1` refreshes expectations after an intended change.
- **Verification.** Rust, WASM-worker and frontend tests cover export, forked opening, refused
  files, opening over an unreadable save, link encoding, confirmation rules, clipboard refusal
  and command logs. `scripts/check.sh` and the production build pass. A scripted Chrome session
  (Playwright, against both the dev server and the production build) checked: a developed town
  survives a reload with the same population; a friend opens the link on a first visit without
  a question, gets the same city paused, keeps running and editing it, and keeps it after a
  reload while the sender's city is unchanged; opening a file over an existing city asks first
  and Cancel keeps it; a pasted link asks first; damaged files and links change nothing; New
  city asks, saves at once and survives a reload; the Command log button appears only in
  development. A real 60×60 town with about 140 residents is a 6.8 MB save and a 0.6-million-
  character link.

### First slice: one city with autosave — implemented and verified

Simulation, worker, storage and UI checks pass, including identical continued evolution after
restoring a developed town. `scripts/check.sh` and the production build pass. A scripted
browser smoke test (headless Chrome against `vite preview`) also passes: a zoned town of about
900 residents with moving cars saves, reloads paused with identical HUD statistics and
buildings, survives a cancelled New city unchanged, is replaced by a confirmed New city that
saves at once, and a damaged slot shows an error, stays unoverwritten and recovers only through
confirmed New city.

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
4. **Integration checks and documentation — complete.** Extend the existing Rust, WASM-worker and frontend
   test harnesses. Update the root README only once saving is implemented; mark this plan's
   completed steps and ADR 0033's implementation status accurately. Run `scripts/check.sh`,
   the production build and a browser save/reload/new-city smoke test (passed).

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

## M4.5 – A great road tool

Building is the core fantasy, and M5's build costs will reward precise tools, so the road tool
gets a Cities: Skylines 2–level pass first. The sim plans every snap, grid and undo, as it
already plans roads: answers stay deterministic and testable, and M6 councilors can reuse the
same builders through ordinary commands. Slices ship in this order; each is judged in the
browser before the next starts. Slices 3–5 wait for [M4.6](#m46--a-proper-interface), so the
grid tool, upgrade and undo land in the new interface.

1. **Angle snapping and guide lines** ([ADR 0038](adrs/0038-sim-owned-drawing-guides.md)).
   New roads snap to 15° steps (90° and 45° pull hardest) measured from the road they start
   on, or the map axes on empty land. Dashed guide lines line new ends up with nearby road
   ends, along their extensions and perpendiculars. Lengths snap to whole 8 m zone cells.
   Length and angle show beside the cursor while drawing. Where the road joins a road, the
   map shows the angle as an arc, or a square at 90°. Angle, guide and length snapping
   toggle separately; holding Shift turns all three off. Joining existing junctions and roads
   always stays on.
   Straight roads that meet end to end in a line become one road, so no hidden junction pulls
   new roads off their angle ([ADR 0040](adrs/0040-straight-joins.md)).
   Done when: a player can draw a square block and a T-junction exactly at 90° without
   zooming in, and can draw a free-angle road by holding Shift.
2. **Continuous curves — implemented and approved in the browser** ([ADR 0046](adrs/0046-continuous-curves.md)).
   A separate mode starts with start, bend and end clicks; each later click adds a piece that
   leaves the previous one tangentially. The sim derives the bend for preview and build, and
   refuses an end behind the previous piece's outgoing direction. Existing build commands and
   the save format stay unchanged; freehand drag is not planned. Automated tests cover smooth
   joins, refused pieces, preview/build agreement and cancellation. The browser check passed.
   Done when: a winding road of five pieces has no visible kinks.
3. **Grid tool.** Pick a cell size (60, 80 or 100 m), drag the first edge to set the angle,
   then drag out the area in whole blocks. Each grid line is planned like a hand-drawn road:
   it joins and splits existing roads, and lines that would overlap are left out. The preview
   shows exactly what will be built. **ADR** for the grid command.
   Done when: a grid dragged across the edge of a town connects into it with no slivers.
4. **Upgrade.** Click a road or drag along connected roads to switch between street and main
   road; alleys can become streets. Cars reroute rather than despawn. Dragging is
   implemented: drawing a road over whole roads gives them its class, in place, for the
   difference in build cost ([ADR 0048](adrs/0048-drawing-over-roads-changes-their-class.md));
   clicking a single road is not.
5. **Undo and redo** for road edits only, many steps, Ctrl+Z and Ctrl+Shift+Z. Undo removes the
   roads an edit added and rejoins roads it split, as a new edit with new IDs; buildings it
   cleared stay gone. History lives in the sim and is saved with the city
   ([ADR 0039](adrs/0039-road-edit-history.md), proposed).
   Done when: building, upgrading and bulldozing can each be undone and redone, also after a
   reload, and replay tests stay exact.

Done when: someone used to Cities: Skylines lays out a town's roads without fighting the tool.

## M4.6 – A proper interface

**Status: implemented.** All four slices are in place and the done-when checks pass, including
a small town built in the browser with only the new interface. The item pictures add about
60–280 ms at startup (shown in the debug panel).

The interface was still a developer's: one row of about 25 text buttons, a block of monospace
text that mixes player figures with frame timings and key help, and a save panel that never
goes away. The roadmap adds roughly three times as many tools (grid, upgrade, undo, services,
overlays, transit, councilors), so the screen needs a shape that holds them, and it should look
like a game. It runs before M4.5's slices 3–5. Layout and style need no ADR
([adrs/AGENTS.md](adrs/AGENTS.md)); the sim, protocol, saves and replays do not change.

- **Layout.** A slim top bar shows the clock and speed, population, money, power and R/C/I
  demand. Clicking a figure opens its details, and a figure turns red when something is wrong:
  power short, the city in debt, a failed save. The bottom build menu has three levels:
  categories in build order (Roads, Zones, Power; later Services, Transit, Councilors), picture
  items (street and main road; six zone brushes and unzone; coal, solar, wind and power line)
  and options (straight, curve, continuous, snaps, a one-line hint). Bulldoze and Views sit
  apart on the right. Save, share, sound and the development command log move into a ☰ menu;
  frame and tick timings, stuck cars and failed routes move into a debug panel opened with the
  backtick key. With no tool selected, only the top bar and the category bar show.
- **One list.** Categories, items, options, labels, keys, tooltips and picture models come from
  a single list in the code, and the menu is built from it, so a new tool is one entry and one
  model. The road, zone and power tools keep their canvas handling and sim commands; the menu
  owns which button is selected.
- **Look.** Dark frosted glass panels, a crisp sans-serif, blue for the selection and red only
  for problems. Item pictures are drawn in the browser from the game's own models; category
  icons come from small low-poly models in the same style (road piece, zone slabs, lightning
  bolt, bulldozer, binoculars).
- **Behaviour.** Every current key keeps its meaning. Views combine as today: one ground overlay
  at a time (air, ground, noise, land value), with traffic colours and power markers separate.
  Picking a power tool shows the power view until the tool is put away.
- **Out of scope:** title screen, settings, notifications, statistics graphs, keybinding
  editor, first-run hints, locked era items and citizen portraits.

Slices, each checked in the browser against the approved look before the next starts:

1. **Frame and look.** The style, the top bar with detail panels and warnings, the ☰ menu and
   the debug panel. The old HUD text and city panel go; the old toolbar stays, restyled.
2. **Build menu.** Categories, items with text labels, the options row and hint, Bulldoze,
   Views, and the power view with power tools. The old toolbar goes.
3. **Pictures.** Item pictures and category icons drawn from models.
4. **Inspector card.** The citizen and building inspector as a card in the same style.

Done when: every action in today's interface is reachable and every key works as before; with
no tool selected only the top bar and category bar show, and developer numbers appear only in
the debug panel; a test shows that one list entry adds a working item; power turns red when
the town needs more than it has and its panel explains why, and money turns red in debt; item
pictures come from the game's models; every control works from the keyboard and has an
accessible name; `scripts/check.sh` passes; and a small town can be built in the browser using
only the new interface.

## M5 – A game loop

Goals and consequences, kept light so they don't crowd out building.

- Money: build costs, taxes, upkeep (implemented, [ADR 0036](adrs/0036-money-build-costs-taxes-upkeep.md)).
  A fresh city starts on an empty map with $50,000. Roads cost money to build by length and
  class and refuse to build when the city cannot pay; the road tool quotes the cost. Every road
  costs upkeep, and residents and filled jobs pay taxes, per clock hour. A flat $1,000 grant per
  clock hour pays for early roads before taxes can. Upkeep beyond income can push the
  balance into debt, which only stops new roads. The top bar shows the balance and net income;
  its money panel shows taxes, grant and upkeep.
  Bulldozing, zoning and sim-built alleys are free. Tax rates are fixed for now.
- **First slice: pollution and land value — implemented** ([ADR 0037](adrs/0037-pollution-and-land-value-steer-growth.md)).
  Industry, traffic and (since ADR 0044) coal plants make air pollution, ground pollution and
  noise, which lower land value.
  Homes, and shops a little, grow first on high land value; industry ignores it. Nothing
  standing is closed or emptied. It needs only buildings and traffic, so it comes before
  services.
  - Sim: a pollution grid aligned to the zone grid (4×4 zone cells per cell). Smoothed
    traffic flow per road and ground pollution per cell build up over time. Air, noise and
    land value are rebuilt on a fixed sim-time cadence, not every tick. All are saved. The
    ADR 0025 growth check gains a land value threshold for residential (strong) and commercial (mild).
  - Saves: version 3 holds money and pollution. Older saves and links are refused, like other
    version changes (ADR 0033). Bless replay `expect` lines once.
  - Display: overlays for air, ground, noise and land value; display-only haze and stained
    ground. Add the snapshot format to CLAUDE.md with the Rust writer and renderer reader.
  - Perf: compare `traffic_bench` ms/tick at the default size and `--scale la`.
  - Built: the P button cycles air, ground, noise and land value; the plain view keeps a faint
    haze and stain, and factory chimneys puff display-only smoke. At `--scale la` (about
    600,000 cells) an update costs about 10 ms once every 100 ticks, about 0.1 ms a tick on
    average; the default bench is unchanged. In a test town, the strip beside a factory
    district (and a main road) grew 5 homes against 11 to 19 in strips farther away.
    Not yet checked by hand: the full browser save and reload flow.

  Done when: in a town with a factory district and a busy main road, new homes visibly fill
  the quiet, clean blocks first and the dirty ones only when housing demand is high; an
  old factory site stays tainted for a few sim days after rezoning; the overlays explain
  why.

- **Power — implemented** ([ADR 0041](adrs/0041-power-grids-follow-roads.md),
  [ADR 0042](adrs/0042-roadside-pieces-snap-to-roads.md)). Roads carry power. Coal snaps beside
  roads and offers industrial jobs; off-road solar and wind connect through transmission lines.
  Plants and lines remain free, alongside the road economy. Solar follows daylight, wind
  varies from the seed, and coal output is constant regardless of staffing.
  - Lots need both sufficient R/C/I demand for their land value and available power.
    Unpowered buildings keep their residents and workers but fill no vacancies. Starter
    power supports a hamlet before the first plant; shortages leave newer buildings dark.
  - Plants clear their land; line corridors remove buildings and block growth until removed.
    Lines snap to plant terminals, roadside poles and other line ends.
  - The HUD reports supply, use and waiting lots. Unpowered buildings have markers; the power
    view colours grids. Detailed low-poly models show each plant type without changing state.
  - Save version 4 combines money, pollution and power. Older saves and links are refused.
    Coal plants puff display-only smoke from their chimney; plants do not yet add costs.
  - Coal plants pollute like a large factory: air, noise and ground pollution lower land
    value around them; solar and wind stay clean
    ([ADR 0044](adrs/0044-coal-plants-pollute.md)).

  Done when: a player grows a village on starter power, sees why growth stops, adds a plant,
  and connects an off-road wind farm with a line.

- **Fire** ([ADR 0053](adrs/0053-fire-stations-send-trucks-through-traffic.md)). Roadside fire
  stations cover buildings within a driving distance along roads and raise their land value.
  Above about 1,000 residents, grown buildings occasionally catch fire (industry and dense
  more often). A station sends a real truck through traffic; one that arrives in time saves
  the building, otherwise it burns down, its people rehome or leave, and ruins stand for a
  day before the lot regrows. A city setting turns fires off. Police, health and schools
  later reuse the same shape.

  Done when: a town nearing 1,000 people sees the warning, builds a station, watches a truck
  cross town and save a house, loses a building in an uncovered or jammed area, and can turn
  fires off.

- Remaining services with coverage areas: water (in progress), roads maintenance, then
  police, health and schools.
- Citizen happiness from services, traffic and pollution, feeding back into demand and growth.
  The citizen inspector shows each person's mood and complaints.
- Data overlays: traffic, coverage, land value, happiness.
- Two modes:
  - **Sandbox:** never fails. Problems slow growth or make people move away, but there is no game
    over.
  - **Scenarios:** a set start and goal ("reach 5,000 with no gridlock", "rescue a bankrupt
    town") that you can fail. Built on M4 save files: a scenario file pairs an unchanged
    ADR 0033 save payload (the start state) with a separate goals section, so goals never
    enter the sim's save schema. The scenario file format needs its own ADR.

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
