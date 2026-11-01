# sim-game

A city sim in the spirit of Cities: Skylines. The simulation is a deterministic Rust crate compiled to
WASM and run in a Web Worker; the frontend is TypeScript + Three.js.

## Run

Needs Rust (with the `wasm32-unknown-unknown` target), `wasm-pack`, and Node 24+.

```bash
cd web && npm install && npm run dev
```

Controls: left-drag pans, right-drag orbits, wheel zooms to cursor. `R` toggles straight roads
(click start, click end). `V` toggles curved roads (click start, click bend control, click end).
The bend control pulls the curve toward it; it is not a point the road passes through.
Both construction tools continue from the last successful end. `B` toggles bulldoze: hover to
highlight a whole road between junctions, then click to remove both directions. All tools work
while paused. Esc or right-click cancels. Space pauses, 1/2/3 set speed.
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
shapes and share side walls; industry grows as sheds with space around them. Density changes
building height. Very small or awkward lots can stay open. Growth pauses with the simulation.
This is quick visual growth for testing zoning, not demand-driven development yet.
Repainting or unzoning keeps existing buildings and their original use, while unzoning stops
new construction. A road edit that removes a lot removes its building immediately. Citizens,
demand, and gradual redevelopment come in later issues.

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

Architecture decisions: [docs/adrs](docs/adrs/README.md). Plan: [docs/ROADMAP.md](docs/ROADMAP.md).

## Benchmarks

```bash
cargo run --release -p sim --example traffic_bench -- --agents 50000 --grid 60
cargo run --release -p sim --example traffic_bench -- --scale la   # ADR 0012 scale target
```
