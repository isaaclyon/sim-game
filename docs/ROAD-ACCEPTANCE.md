# Roads milestone: supporting acceptance evidence

Issue [#13](https://github.com/isaaclyon/sim-game/issues/13) remains **not fully accepted**.
The existing generated irregular network passes the proposed traffic threshold on two seeds.
A human-drawn network and interactive road-tool usability remain unverified.

## Scope and reproduction

Measured simulation revision: `c28e1beeb59555f6a4e6ab02e56d39bf59e6dcda`.
Runs use the existing `crates/sim/examples/traffic_bench.rs`, native release mode on macOS
arm64 with Rust 1.97.1. No simulation or frontend behavior changes accompany this report.

```sh
cargo run --release -p sim --example traffic_bench -- --layout organic --agents 50000 --ticks 36000 --warmup 0 --seed 1 --max-despawns 9
cargo run --release -p sim --example traffic_bench -- --layout organic --agents 50000 --ticks 36000 --warmup 0 --seed 2 --max-despawns 9
```

The proposed meaning of **near-zero** is at most nine stuck despawns over one simulation hour.
This follows [ADR 0009](adrs/0009-intersection-rules.md), which specifies under ten per hour
at 50,000 agents on the generated grid and a scripted hand-drawn layout. It is an explicit
proposal for interpreting #13, not a new acceptance decision. Each stuck despawn still counts
as a traffic-rule failure, as the ADR requires.

The `organic` fixture is **generated, not hand-drawn**. It constructs 34 concentric rings,
with `6r` candidate junctions on ring `r`, nominal radial spacing of 110 metres, seeded angular
and radial jitter, and a 12% chance of omitting each ring edge. Each junction connects to its
nearest junction on the inner ring. It submits straight `Command::BuildRoad` commands, all
with class `Street`; there are no main roads, Bézier curves or deletion commands. Endpoints
join existing junctions, but these two generated networks do not exercise intersection splitting:
review replay confirmed that each build command adds exactly one road. This exercises irregular
junctions and missing links, rather than a regular-grid benchmark, but cannot alone satisfy
the literal hand-drawn acceptance check or cover every road class.

Both runs spawn 50,000 agents and advance 36,000 ticks at the default 0.1 seconds per tick,
with no warmup: one simulation hour including startup. Counters cover the whole run.
Agents also park or wait for routes and entry; 50,000 spawned agents does **not** mean 50,000
cars driving continuously. Driving counts below are sampled after ticks 1, 101, …, 35,901
(every ten simulation seconds); the mean is integer-truncated and the peak is a sampled peak.

## Measurements

| Measurement | Seed 1 | Seed 2 |
|---|---:|---:|
| Nodes | 3,571 | 3,571 |
| Roads | 6,669 | 6,726 |
| Mean driving agents (sampled) | 45,754 | 45,683 |
| Peak driving agents (sampled) | 48,587 | 48,526 |
| Completed trips | 68,762 | 71,641 |
| Paths found | 116,269 | 119,036 |
| Failed paths | 0 | 0 |
| Stuck despawns | 3 | 0 |
| Detours | 454,605 | 431,613 |
| Mean / p99 tick (ms) | 0.81 / 2.39 | 0.86 / 2.65 |
| Maximum tick (ms) | 46.78 | 35.80 |
| Timed simulation wall time (s) | 29.2 | 31.0 |
| Threshold assertion exit code | 0 | 0 |

These are native timings, not browser/WASM performance measurements. Repository checks ran
concurrently for part of the measurement, so the timings are supporting observations, not an
isolated performance comparison. Network construction and compilation are outside timed ticks.

## Automated checks and remaining acceptance work

`scripts/check.sh` passes: full-history secret scan, Rust formatting, host and WASM clippy,
105 simulation tests, WASM build, 26 web tests, Biome and TypeScript checks. The first run
failed because this fresh worktree lacked web dependencies; `npm ci --prefix web` installed
the existing lockfile dependencies, and the full check then passed (Node 24.22.1).

Existing `crates/sim/tests/curves.rs` tests cover snapping/splitting around curves, whole-curve
bulldozing, read-only previews matching construction, refused edits and traffic replay.
`web/tests/road-tools.test.mjs` checks mocked event handlers for curve chaining, refusal,
mode switching, cancellation and delayed previews. These are automated checks, not observed
mouse interaction, WebGL rendering or a judgment that the tools feel usable.

Interactive inspection is blocked: `terminal-browser ls --json` reports
“Ghostty is not running, so there is no window to script.” No browser interaction or manual
usability result is claimed. The milestone owner still needs to approve the threshold,
validate a genuinely human-drawn network for one hour, clarify whether 50,000 means the
agent population or simultaneously driving cars, and inspect snapping, curves and bulldozing
in the browser. This report does not close #13.
