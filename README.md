# sim-game

A city sim in the spirit of Cities: Skylines. The simulation is a deterministic Rust crate compiled to
WASM and run in a Web Worker; the frontend is TypeScript + Three.js.

## Run

Needs Rust (with the `wasm32-unknown-unknown` target), `wasm-pack`, and Node 24+.

```bash
cd web && npm install && npm run dev
```

Controls: hold W/A/S/D or left-drag to pan, hold Q/E to turn left/right, right-drag orbits,
wheel zooms to cursor. Held keys move relative to the view, speed up smoothly, glide to a stop
when released, pan faster when zoomed out, and also work while paused or with a tool on. `R` toggles straight roads
(click start, click end). `V` toggles curved roads (click start, click bend control, click end).
The bend control pulls the curve toward it; it is not a point the road passes through.
Both construction tools continue from the last successful end. `B` toggles bulldoze: hover to
highlight a whole road between junctions, then click to remove both directions. All tools work
while paused. Esc or right-click cancels. Space pauses, 1/2/3 set speed.
Ambient sound follows the camera: quiet, brief engine sounds near moving cars and birds over
open land by day. Sparse traffic stays quiet; no moving cars means no traffic sound. Buildings
never produce a static noise loop. Zooming out makes sound quieter, not louder. Sound starts
on the first click or key press; `N` or the sound button toggles it, and the choice is remembered.
A new city starts on an empty map with $50,000. Roads cost money to build: $10 per metre for a
street and $25 for a main road. The road tool shows what the previewed road costs, and refuses
a road the city cannot pay for. Every road also costs upkeep each in-game hour, and residents
and filled jobs pay taxes. The HUD shows the balance and the hourly taxes and upkeep. Upkeep
can push the balance into debt, which stops new roads until taxes bring it back. Bulldozing
and zoning are free ([ADR 0036](docs/adrs/0036-money-build-costs-taxes-upkeep.md)). Saves
from before power was added (versions 1–3) no longer open; start a New city.
The HUD shows the sim's day clock and speed (`Day 1   06:00   1×`). The game starts on Day 1 at
06:00, and each sim second advances the clock six seconds, so a day lasts 4 real hours at 1× and 24
minutes at 10×. Speed changes how many sim seconds pass per real second; pausing stops the clock.
Lighting follows that same clock: warm dawn and dusk fade into daylight and a blue night.
Building windows, shopfronts and vehicle head/tail lamps glow after dark; pausing holds the
lighting too. Buildings and trees cast sun shadows onto the ground and roads. Shadows cover
a bounded area around the camera focus so they stay detailed when viewing a large city;
vehicle lamps use emissive faces rather than individual light sources.
Cars whose remaining route uses a deleted road park and retry; other traffic continues.
The zone brush paints residential (`Z`), commercial (`X`) or industrial (`C`); press the key
again to switch between low and high density, or use the tool bar. `U` unzones. Click inside a
block (land enclosed by roads, outlined on hover) to zone all of it, or drag a rectangle to
paint just that (left-drag paints instead of panning while the brush is on). The sim cuts
zoned blocks into lots facing their roads, and adds alleys or connector streets where a block
is too deep to reach from its edges; these are ordinary roads you can bulldoze.
Zoning colors and lot outlines appear only while a zoning brush (including unzone) is active.
Outside zoning mode, developed lots show lawns, garden trees and beds, paths, shop planters,
and factory yards. Houses mix front gardens, simple lawns and
street-connected driveways where a full parking space fits. Narrow paths align with their
front doors. Backyards vary between open lawn, a patio with seating, vegetable beds and a tree.
Each house keeps its yard choices across reloads. Residential boundaries mix joined hedges,
warm-white picket fences and open lots. Factory yards vary too: asphalt, concrete or gravel
ground; open metal fences, solid block walls or no boundary; a truck court across the loading
doors or one drive to a door; and one main yard use (pallets of goods, shipping containers,
storage tanks or a row of staff parking), sometimes with a skip. Enclosed
sides meet at corners, with openings for paths, driveways, buildings and roads. Shops keep
open frontages paved in sand, grey slab or brick, with planters, cafe tables, cycle hoops and a
bench, or bollards. Shops set well back from the street park cars in front instead: one asphalt
car park from the shop to the street, with no bays marked in front of the door. Small shops keep
green rear space with
one use: trees, a terrace with parasols, open lawn with a bench, or (most often) a storage yard
with a shed, pallets of stock and bins. Dense shops may have room only for shallow entrance
planters and a bin corner.
These details are decoration, not parking or growth rules. They follow the standing building
through rezoning and disappear with it. Vacant zoned land stays grassy; painting still clears
wild trees and fields.

Buildings start appearing after a few simulation seconds and fill suitable zoned lots within
about a minute at normal speed. Low-density homes have yards. Low-density commerce grows
small, shallow shops, mostly one storey, with individual doors and awnings, mixed flat and
pitched roofs, and occasional attached pairs. Most shops keep side gaps and rear space.
Apartments and high-density commerce follow lot shapes and share side walls
([ADR 0031](docs/adrs/0031-small-low-density-shops.md)). Newly zoned industrial land mixes small workshop lots, wider
warehouses and occasional factory-sized sites, with varied hall sizes, heights, yards and roof
colours. Larger sites need adjoining land of the same industrial zone; curves and small blocks
may leave smaller lots. Industrial halls stay rectangular; corners too narrow for a hall stay
open rather than growing pointed sheds. Repainting preserves existing lot sizes and buildings. Density changes
building height. Low-density shops provide one job per 25 m² of footprint, with at least two
jobs; a 200 m² shop supports eight jobs, and upper floors do not add jobs. High-density commerce retains one job per 30 m² across
its floors. Industry provides one job per 30 m² of footprint at low density (at least five)
and per 10 m² at high density (at least ten), independent of hall height. This lets fewer
industrial lots support the town; demand shares stay unchanged. Very small or awkward lots can stay open. A partly painted lot also stays
empty if unzoned cells outnumber or tie its most common zone; click to fill the whole block
to zone those gaps. Growth pauses with the simulation.
Growth needs enough R/C/I demand for the lot's land value and enough power.
Repainting or unzoning doesn't bulldoze. Existing buildings keep their original use while
they empty out: workers lose their jobs at once, and households move to a free home elsewhere
or leave town, one every few seconds. An empty building then disappears and the new zone grows
in its place; unzoned land stays vacant. Painting the original zone back before that stops
the closure. A road edit that removes a lot removes its building immediately
([ADR 0023](docs/adrs/0023-gradual-closure-of-rezoned-buildings.md)).

## Power

`G` cycles coal, solar and wind plants; `L` draws transmission lines; `O` toggles the power
view. `P` continues to cycle the pollution views.

Plants and transmission lines are free to build and maintain. Roads carry power; lines
connect remote plants or separate road networks. Coal snaps beside a road, faces it and
provides constant power and industrial jobs. Solar produces only in daylight. Wind output
varies slowly. Staffing does not affect output. Each plant has a fixed connection point
behind it; lines also snap to roadsides and existing line ends.

Starter power supports a small village. When it runs out, lots wait until a plant supplies
power. Existing buildings receive power oldest first; a shortage stops their empty homes
and jobs from filling but does not evict anyone. The HUD reports supply, use and waiting
lots. Unpowered buildings are marked, and the power view colours separate grids.

Placing a plant clears its land. Lines remove buildings they cross and prevent rebuilding
until the line is bulldozed. Bulldozing selects a plant first, then a line, then a road.
Detailed plant models are display-only; power plants do not yet contribute pollution.
See [ADR 0041](docs/adrs/0041-power-grids-follow-roads.md) and
[ADR 0042](docs/adrs/0042-roadside-pieces-snap-to-roads.md).

## Saving

The game keeps one city in this browser, at this site address. **Save city** saves immediately;
autosave saves changes every 60 real seconds, including edits made while paused. The status
shows unsaved changes, the last successful save time or a storage error. Returning to the game
automatically restores the saved city, paused, with its residents, jobs, traffic and scheduled
events intact. Camera position, tools and inspection are not saved.

**New city** asks before replacing the current city and its save. Cancel keeps both. A failed
write keeps the previous saved city; use Save city to retry. If an existing save cannot load,
the game reports the error rather than silently starting over or overwriting it. If another
tab saves first, an outdated tab cannot overwrite it; reload that tab before saving again.

This is not a backup: clearing site data or browser eviction can remove the city. Another
browser, profile or site address has a separate save. **Download** keeps a copy as a `.city`
file. There is no save history or conversion of incompatible save formats yet. Save before
closing if you need the latest changes; tab-close saving is not guaranteed. See
[ADR 0033](docs/adrs/0033-single-city-state-saves.md).

## Sharing

**Copy link** copies a link that contains the whole city; nothing is uploaded. **Download**
saves the city as a `.city` file to send instead, which also works for cities too big for a link.
A friend opens the link, or uses **Open file…**, and gets their own copy, paused where you
left off. Their edits stay in their browser and never change yours. Opening a shared city
replaces the city in that browser, so the game asks first whenever a city would be lost. A
damaged link or file changes nothing. Both people need a build that reads the same save version.
See [ADR 0035](docs/adrs/0035-share-cities-as-forked-copies.md).

## Inspection

With no tool active, click a building to see its use, density and live counts. Homes show
resident capacity and how many people live there, including residents who are currently out.
Shops and factories show total, filled and unfilled jobs; filled jobs count assigned employees,
not people physically at work. Closing buildings keep their original use and show their status.
Demolition clears the selection. Counts refresh while selected, including after paused edits.

Click a car or walker to inspect its citizen instead; `F` toggles following that citizen.
Buildings and citizens share one panel. Click empty ground, press Esc or use Close to dismiss it.
Construction and zoning tools keep their normal click behavior. Inspection is read-only and
shows counts only, not resident or employee lists.

## Citizens

Normal play starts with no cars. Completed homes receive households gradually, even without
jobs. The population counter counts residents, including people who are not travelling.
Next to it, the HUD shows how many residents have no job and what share of the population
that is (0% when nobody lives in town). A job at a workplace that is temporarily cut off by
road edits still counts as a job.
Houses hold one household of 1–7 adults; apartments provide several separate homes. Home
capacity scales with usable floor area, while household size uses a weighted distribution
that keeps small households common. [ADR 0021](docs/adrs/0021-citizens-households-and-trips.md)
records the capacity formulas and lifecycle contract.

Residents seek reachable jobs in commercial and industrial buildings. Jobs are limited;
industrial halls count as one usable floor. Daily trips go to work, optionally shop, then home.
Unemployed residents sometimes shop during the day. Only commercial buildings offer shopping.
First activities are staggered shortly after moving in; later departures follow the day clock.
Time spent at work or shopping starts on arrival, and late trips may finish the next day.

Every normal-play car belongs to one resident's trip. Failed routes wait and retry. Road edits
and stuck-car recovery return the traveller to their last activity location, never claim arrival.
Trips between buildings sharing an access junction complete without a visible car. Demolished
workplaces release jobs. Demolished homes relocate whole households to suitable vacant homes,
or residents leave town if none exists. Repainting blocks new household/job/shop assignments
but preserves existing residents, jobs and previously assigned trips until gradual closure is
implemented. Restoring the exact original zone allows assignments again. The explicit
`SpawnAgents` / worker `spawn` path still creates synthetic traffic for benchmarks, not gameplay.

## Roads

Curved roads use quadratic Bézier centrelines. Cars travel by distance along the curve and
face its local direction. Junction rules use the direction at each end, not the straight line
between endpoints. Splitting a curve preserves its shape and existing road/lane ids.

The sim calculates previews, snapping, crossings and rendered line segments. White preview
markers join existing nodes; yellow markers split roads. A red preview means the build is
refused. Construction refuses overlaps, pieces under 10 m, loops, repeated crossings of the
same road, a second road between the same two junctions, and invalid or cusp-shaped curves. Coordinates must be finite and within ±1,000 km.
A refused build changes nothing and keeps the selected start and bend control for another try.

Internally, curves share a distance lookup table between their two lanes. Sampling uses a
2 cm chord-deviation target; position and heading queries evaluate the Bézier itself. Straight
roads keep their direct geometry queries and do not allocate a distance table. The road
snapshot keeps its five-float segment format, with multiple segments per curve. Preview
samples use join code -1 between actual junctions.

Roads render as asphalt ribbons with simple pads at shared endpoints: streets are eight metres
wide, main roads twelve. Press M (or the main road button) to build main roads with either road
tool. Main roads are faster, and long trips ride them in the middle (ADR 0017).
The view preserves the simulation's right-hand traffic.

Run `scripts/check.sh` for the full checks, including road geometry tests and replay
regression tests: each `crates/sim/tests/replays/*.cmdlog` session must rebuild the same city
([ADR 0034](docs/adrs/0034-command-log-replay-tests.md)). Record a new session in `npm run dev`
with the city panel's **Command log** button, put it in that folder and run
`BLESS_REPLAYS=1 cargo test -p sim --test replay` to add its expected result. Rerun the same
command after an intended gameplay change, and review the changed `expect` lines. To run just those
frontend tests, use `cd web && npm test`.

## Layout

- `crates/sim` – the simulation. No I/O, no clock, no randomness except the seeded `Rng`.
- `crates/sim-wasm` – wasm-bindgen wrapper around `sim`.
- `web/` – Vite app. `sim.worker.ts` owns the sim; `main.ts` draws snapshots.
- `scripts/` – `check.sh` runs every CI check, including the 500-line limit on code files.

Architecture decisions: [docs/adrs](docs/adrs/README.md). Plan: [docs/ROADMAP.md](docs/ROADMAP.md).

## Benchmarks

```bash
cargo run --release -p sim --example traffic_bench -- --agents 50000 --grid 60
cargo run --release -p sim --example traffic_bench -- --scale la   # ADR 0012 scale target
```
