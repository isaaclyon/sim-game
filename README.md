# sim-game

A city sim in the spirit of Cities: Skylines. The simulation is a deterministic Rust crate compiled to
WASM and run in a Web Worker; the frontend is TypeScript + Three.js.

## Run

Needs Rust (with the `wasm32-unknown-unknown` target), `wasm-pack`, and Node 24+.

```bash
cd web && npm install && npm run dev
```

Controls: left-drag pans, right-drag orbits, wheel zooms to cursor. `R` toggles the road tool
(click, click, ... Esc to stop). Space pauses, 1/2/3 set speed.

Roads render as eight-metre-wide asphalt ribbons with simple pads at shared endpoints.
Streets and avenues share one asphalt color; their simulation speed limits are unchanged.
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
