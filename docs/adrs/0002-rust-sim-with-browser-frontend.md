# 0002. Rust sim compiled to WASM, browser frontend

- Status: Accepted
- Date: 2026-10-30

## Context

The goal is a Cities: Skylines-style builder with individual agents and freeform roads, built to
scale (50k agents now, 200k+ later). It should be easy to share, and a Steam release should stay
possible. Agents write the code, so language familiarity for the human doesn't matter; mature
tooling and runtime speed do.

Options considered:

- **Unity / Unreal.** Proven for this genre, but heavy editors and binary scene files are a poor
  fit for agent-written code, and sharing means downloads.
- **Godot + godot-rust.** Good editor and native export, but the sim/engine boundary is more
  friction and sharing still means downloads.
- **TypeScript only.** Fastest to build, but the sim would hit a performance ceiling well below
  our agent targets.
- **Rust sim + browser frontend.** Native-speed sim, shareable via a link, plain-text code
  throughout.

## Decision

- The simulation is a plain Rust library (`crates/sim`) with no engine or platform dependencies.
- It is compiled to WASM (`crates/sim-wasm`, wasm-bindgen) and run in a Web Worker.
- The frontend is TypeScript + Three.js, built with Vite (`web/`).
- For Steam, wrap the web build in Electron (or Tauri) later.
- If the browser becomes the limit, the sim crate can move unchanged to a Godot frontend via
  godot-rust. That is the escape hatch, not the plan.

## Consequences

- Benchmarks: 50k agents at ~0.5 ms/tick native and ~1.2 ms/tick in WASM; 200k agents at
  ~8 ms/tick native.
- Sharing is a URL. No install, no engine license.
- WASM is single-threaded for now. Multi-threading needs SharedArrayBuffer and cross-origin
  isolation headers, which we defer until a profile demands it.
- We build our own editor tooling (road tool, zoning brushes) instead of getting it from an engine.
