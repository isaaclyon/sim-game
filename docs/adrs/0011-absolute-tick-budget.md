# 0011. Absolute tick budget for traffic

- Status: Accepted
- Date: 2026-10-31

## Context

ADR 0009 allowed `traffic_bench` ms/tick to rise by up to 30% over the pre-change baseline. That
baseline was cheap partly because it was gridlocked: tens of thousands of stuck cars per
sim-hour, and a stuck car costs almost nothing. The finished junction rules, routing (ADR 0010)
and queue flow at 50k agents run at about 0.6-0.8 ms/tick native, 2.5-3.5x the baseline, while
completing 4-5x the trips on the grid with 0-3 stuck despawns. Cost per completed trip is
close to the baseline's. A relative target rewards a sim for doing less.

## Decision

This replaces the ms/tick line in ADR 0009's Consequences. The rest of 0009 stands.

- The traffic budget is absolute: mean ms/tick in `traffic_bench` at 50k agents, native release
  build, stays under **2 ms** on both the grid and the hand-drawn layout.
- Judge a change by its cost against what it buys (trips completed, stuck despawns), not by
  its ratio to the previous build.

## Consequences

- 2 ms native is about 5 ms in WASM (ADR 0002 measured WASM at about 2.4x native), half the
  10 ms tick budget for 10x game speed. The rest is left for the systems still to come.
- Today's branch sits at 0.6-0.8 ms, so there is room, but not for careless growth.
- Profile at the time of writing: car following 29%, route search 23%, exit check 16%, tick
  overhead 17%, junction requests 9%. Road-class routing (M2) is the expected next cut.
- The 200k+ agent target (ADR 0002, M7) needs its own measurement and budget when it comes up.
