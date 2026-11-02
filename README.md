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
Ambient sound follows the camera: traffic hum over busy roads, city noise among buildings and
walkers, and birds over open land by day. It starts on the first click or key press; `N` or the
sound button turns it on and off, and the choice is remembered.
The HUD shows the sim's day clock and speed (`Day 1   06:00   1×`). The game starts on Day 1 at
06:00, and each sim second advances the clock six seconds, so a day lasts 4 real hours at 1× and 24
minutes at 10×. Speed changes how many sim seconds pass per real second; pausing stops the clock.
Cars whose remaining route uses a deleted road park and retry; other traffic continues.
The zone brush paints residential (`Z`), commercial (`X`) or industrial (`C`); press the key
again to switch between low and high density, or use the tool bar. `U` unzones. Click inside a
block (land enclosed by roads, outlined on hover) to zone all of it, or drag a rectangle to
paint just that (left-drag paints instead of panning while the brush is on). The sim cuts
zoned blocks into lots facing their roads, and adds alleys or connector streets where a block
is too deep to reach from its edges; these are ordinary roads you can bulldoze.

Buildings start appearing after a few simulation seconds and fill suitable zoned lots within
about a minute at normal speed. Low-density homes have yards; apartments and shops follow lot
shapes and share side walls. Newly zoned industrial land mixes small workshop lots, wider
warehouses and occasional factory-sized sites, with varied hall sizes, heights, yards and roof
colours. Larger sites need adjoining land of the same industrial zone; curves and small blocks
may leave smaller lots. Industrial halls stay rectangular; corners too narrow for a hall stay
open rather than growing pointed sheds. Repainting preserves existing lot sizes and buildings. Density changes
building height. Very small or awkward lots can stay open. A partly painted lot also stays
empty if unzoned cells outnumber or tie its most common zone; click to fill the whole block
to zone those gaps. Growth pauses with the simulation.
This is quick visual growth for testing zoning, not demand-driven development yet.
Repainting or unzoning doesn't bulldoze. Existing buildings keep their original use while
they empty out: workers lose their jobs at once, and households move to a free home elsewhere
or leave town, one every few seconds. An empty building then disappears and the new zone grows
in its place; unzoned land stays vacant. Painting the original zone back before that stops
the closure. A road edit that removes a lot removes its building immediately
([ADR 0023](docs/adrs/0023-gradual-closure-of-rezoned-buildings.md)). Demand comes in a later issue.

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

Run `scripts/check.sh` for the full checks, including road geometry tests. To run just those
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
