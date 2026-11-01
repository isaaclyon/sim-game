# 0024. Per-driver route noise

- Status: Accepted
- Date: 2026-11-01

## Context

ADR 0010 prices each lane at its free-flow travel time plus a fixed delay per queued car
(`QUEUE_DELAY_S`). Every driver sees the same prices, so on an empty network every car making
the same trip takes the same route, and on a grid every car breaks ties between equal routes
the same way. The roadmap (M2) asks for a small seeded random change per driver so traffic
spreads out and route choice looks human. ADR 0010 and ADR 0017 expected it to replace or
retune the queue delay.

## Decision

This replaces the **Cost** bullet of ADR 0010. The rest of 0010, and ADR 0017, stand.

- **Cost = travel time × driver taste + queue delay.** In `Pathfinder::find`, each lane's
  free-flow time is multiplied by `1 + ROUTE_NOISE × u`, where `u` in `[0, 1)` is a hash of
  the driver and the lane. `ROUTE_NOISE` is 0.2: a driver sees a lane as up to 20% slower.
- **The taste never lowers a cost**, so the straight-line heuristic stays a lower bound
  before its 1.5x weight.
- **It is fixed per driver and lane.** The driver key is a SplitMix64 hash of the world seed
  and the agent ID. A vehicle slot belongs to one citizen for life (ADR 0021), so a citizen
  keeps the same habits on every trip. Drivers differ from each other, not trip to trip.
- **Hashed, not drawn.** The noise comes from `rng::mix`, never from the gameplay `Rng`, so
  it stores nothing per driver and does not shift any other random stream. Same seed, same
  routes.
- **Queue delay stays at 1 s per car.** Halving it cost 5-7% of trips on both test layouts.
- **Detours use plain travel times.** A detour reacts to one blocked lane; taste adds nothing
  there.

## Consequences

- Drivers making the same trip across a grid now split over the routes that tie, and near
  ties go different ways for different drivers.
- `traffic_bench`, 50k agents, one sim-hour, three seeds: trips completed and stuck despawns
  are within run-to-run noise of the old cost on the grid (80-82k trips, 0-3 stuck) and the
  hand-drawn layout (68-72k trips, 0-3 stuck). LA scale (`--scale la`) is also unchanged
  (about 31k trips, 11-16 stuck): its jams come from main-road capacity, which noise does not
  add. Multi-lane roads (#52) remain the fix for that.
- Route searches expand more nodes, because the heuristic is looser against the raised
  costs: about 5% more on the grid, 12% on the hand-drawn layout and 20-30% at LA scale.
  Mean ms/tick at 50k agents rose by about 0.05-0.1 ms, well inside ADR 0011's 2 ms budget.
- Routes are less optimal by design: a driver may take a route up to about 20% slower in
  free-flow time than the best one, before the 1.5x heuristic weight.
- Tests that need one exact route should call `find` with no driver (`None`).
