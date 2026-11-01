# 0010. Congestion-aware routing and detours

- Status: Accepted; long trips search only main roads in their middle, per [0017](0017-road-classes-and-main-road-routing.md); cost superseded by [0024](0024-per-driver-route-noise.md)
- Date: 2026-10-31

## Context

ADR 0006 routes every trip by free-flow travel time with an admissible A* heuristic. ADR 0009's
junction rules stop most gridlock, but at 50k agents rings of full lanes still form. Every car
in a ring wants the lane ahead, and every car on the network picks the same fastest route, so
avenues fill while parallel streets sit empty. On the 60x60 grid, early versions of this work
still left hundreds of stuck despawns per sim-hour, against ADR 0009's target of under 10.

## Decision

This replaces the **Pathfinding** bullet of ADR 0006. The rest of 0006 stands.

- **Cost = travel time + queue delay.** Each lane costs its free-flow travel time plus a fixed
  delay per car queued on it (`QUEUE_DELAY_S`), read when the search runs. New trips avoid
  backed-up lanes, so traffic spreads across the network.
- **Weighted A*.** The straight-line heuristic is multiplied by `HEURISTIC_WEIGHT` (1.5). Routes
  can be up to 1.5x the cheapest cost, in exchange for far fewer expanded nodes. Queue delays
  make the plain heuristic loose, so the unweighted search expanded most of the city.
- **Detours.** A car held at a stop line by a full exit lane for `DETOUR_AFTER_S` looks for a
  way around it, then retries every `DETOUR_EVERY_S`. The search is local: it starts at the
  current junction, avoids the blocked lane, accepts only a first lane with room, stops after
  `DETOUR_MAX_NODES`, and rejoins the old route where that is cheapest. The car keeps its place
  in its queue. One car turning off breaks a ring. If no local detour exists, the car keeps its
  route and the stuck despawn remains the backstop.
- Scratch buffers stay generation-stamped and new trips still share `path_budget`. Detours do
  not use that budget: each one is a bounded local search.
- Everything stays deterministic: delays come from queue lengths in lane-id order, and detours
  run in the fixed order the junction pass visits held cars.

## Consequences

- Stuck despawns per sim-hour at 50k agents dropped to 0 on the grid and 0-2 on the hand-drawn
  layout across three seeds. Trips completed rose about 4x on the grid.
- Routes are no longer guaranteed fastest. Drivers take "good enough" routes and react to
  traffic, which looks more human, but route quality now depends on the tuning values.
- Route cost depends on live queue lengths, so the same trip can route differently at
  different times. Tests that need a fixed route should use an empty network.
- About 300k-400k detour searches run per sim-hour at 50k agents. They are cheap one at a time,
  but they are a new cost that grows with congestion.
- Detours stay on every held car, not only cars in a deadlock. We tried A/B Street's approach:
  follow the chain of blockers and search only when it loops back to the car. In one sim-hour
  it found 44 loops on the grid and 6 on the hand-drawn layout. Nearly all held cars sit in
  long queues behind red lights or crossing traffic, and without detours about 40k of them hit
  the stuck limit. Searches are not wasted either: 99.6% find a detour. The only lever is the
  wait before detouring, which trades speed for trips: 20 s cut ms/tick about 10% and trips
  2-4%; 30 s cut ms/tick about 19% and trips 6-9%.
- Road classes and per-driver random route costs (M2 in the roadmap) are expected to replace
  or retune the flat per-car queue delay.
