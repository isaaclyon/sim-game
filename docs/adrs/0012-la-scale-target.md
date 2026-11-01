# 0012. Scale target: Los Angeles at rush hour, 10x speed

- Status: Accepted
- Date: 2026-10-31

## Context

ADR 0002 aimed at "200k+ agents later" and ADR 0011 left that target without a measurement or
budget. Agent count alone is the wrong yardstick: parked agents cost almost nothing per tick,
so the load is the number of cars driving at once, and the size of the road network they drive
on.

A rough sizing of the City of Los Angeles (3.9M people, about 10,500 km of streets): about 1.5
car trips per resident per day plus 15% for trucks, deliveries and ride-hail, about 9% of a
day's trips in the peak hour, and about 25 minutes per trip at rush hour. That gives about
250k cars on the road at once (likely range 150k-400k), or about 8% of the city's vehicles.
Los Angeles County would be about 600k.

## Decision

- **Scale target:** a city the size of the City of Los Angeles at rush hour: about **250k cars
  driving at once** on about **10,000 km of street**, with citizens (about 4M) costing little
  while they are not travelling.
- **Speed target:** **10x game speed at that scale**, everywhere we can reach it. The browser
  is the binding case: it is single-threaded unless the host sends cross-origin isolation
  headers (ADR 0002), so the target must be reachable on one core. Native builds (Steam) may use
  more cores for extra headroom. Where a platform cannot reach 10x, it caps the top speed it
  offers rather than stuttering.
- **Budget:** a whole tick, all systems, under **10 ms in WASM** at this scale. As in ADR 0011,
  traffic gets half of that, **5 ms in WASM**, and the rest is kept for systems still to come.
- **Measurement:** `traffic_bench --scale la` (a 200x200 grid, 8,600 km of street, 250k agents,
  timed after a 10 sim-minute warmup). For WASM, build the bench for `wasm32-wasip1` and run it
  under Node, which uses the same engine as Chrome.
- ADR 0011's 2 ms native budget at 50k agents stands. This ADR adds a second, larger case.

## Consequences

- Baseline at the time of writing (Apple Silicon, `--scale la`): traffic takes 7.75 ms native
  and 10.25 ms in WASM under Node, with about 243k cars driving. That is 10x speed with no room
  left for other systems, and p99 ticks of about 29 ms in WASM. Traffic needs to get about 2x
  faster to fit its 5 ms share.
- WASM measured about 1.3x native here, not the 2.4x in ADR 0002, so budgets should be checked
  in WASM directly rather than converted from native numbers.
- Cost per driving car is flat from 100k to 250k cars on the same map (about 32 ns), so there is
  no memory cliff at this size. It is about 2.3x the cost on the 60x60 grid, mostly because
  random trips across a larger map need longer route searches. M2's home-work-shop trips should
  bring part of that back.
- Profile at the time of writing (native, steady state): moving cars 28%, route and detour
  search 24%, exit-room checks 18%, junction requests and conflicts 13%, tick overhead 9%,
  signals 4%, finding the waiting car 3%.
- The exit-room share is not a memory problem. Caching each lane's last-car position, speed and
  car count, so the check never touches the queue or the agent arrays, changed nothing
  measurable (6.3 ms before and after, same machine, interleaved runs). The sampled time there
  is likely skid from the queue walk in `Junctions::request` around it.
- Junctions get about 21,000 requests per tick at LA scale: about 1,200 cars let through, 5,500
  stopped by a red light, and 10,000-15,000 refused because the exit is full. Most are the same
  cars asking again every tick. Caching each car's next lane (so counting cars ahead skips each
  car's path) gained 1-2%, within noise. Checking the light before the exit gained about 7% but
  changed outcomes (1.7% fewer trips, fewer detours), so it is a gameplay change, not a free win.
  The large saving would come from cars not re-asking every tick, which is also a design choice.
- Run-to-run noise on one machine is about 5-15% (the same build measured 6.1-7.8 ms across a
  session), so compare builds with interleaved runs, not against numbers in this file.
- Snapshots grow to about 4 MB per tick at 250k cars, so culled or delta snapshots (M7) are
  required, not optional, to reach 10x in the browser.
- Native and WASM runs of the same seed already differ slightly, most likely from math library
  calls such as `hypot` that each platform implements differently. That is allowed by ADR 0004 but matters for M4: a save replayed from a
  command log on a different platform will not match exactly.
