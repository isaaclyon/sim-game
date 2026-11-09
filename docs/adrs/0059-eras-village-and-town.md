# 0059. The city has an era; buildings keep the era they were built in

- Status: Accepted; implemented
- Date: 2026-11-08
- Extends: [0027](0027-display-only-scenery-and-building-kit.md), [0033](0033-single-city-state-saves.md),
  [0014](0014-zone-grid.md), [0017](0017-road-classes-and-main-road-routing.md)

## Context

M6 (issue #44) starts a city as a village (dirt roads, cottages, small shops) that becomes a
town (paved streets, shopfronts, apartments) once enough people live there. ADR 0027 planned
for an era to pick another building `Style` without new sim data, but nothing says which
buildings are from which era, what an era unlocks, or who decides. Later M6 work builds on
the answer: era zones (farms, offices, mixed use, #45), gradual redevelopment of village
buildings (#46), era transit (#47) and one councilor slot per era (#48).

## Decision

- **The city's era is sim state.** `Era` is `Village`, then `Town`, in order. It is saved and
  hashed. A fresh game city (`World::empty`) starts as a village. A world built from an
  existing road network (`World::new`, so the generated grid of tests and benchmarks) starts
  as a town, which keeps everything unlocked there. A fresh world can be started in another
  era (`World::with_era`) for tests and, later, scenarios. Nothing else sets the era.
- **Unlocking.** At the end of every tick, a village with at least 1,500 residents
  (`TOWN_POPULATION`, children included) becomes a town. Eras never go back: a town that
  shrinks stays a town. Unlocking is not a command and not an undo step.
- **What the town era unlocks.** In the village era the sim refuses dense zones (painting or
  filling apartments, stores or heavy industry) and main roads (building, a grid of, or
  changing a road's class to main road). Previews of a locked class plan nothing. Streets,
  low-density zones, unzoning and every other tool stay open. Undo and redo are not checked:
  history only replays edits made in an era already reached.
- **Roads follow the city's era.** Roads store no era. They are drawn as dirt tracks while
  the city is a village and paved in the town era, all at once when it arrives. The road
  snapshot does not change. Dirt does not change speeds or capacity.
- **Village roads cost half.** In the village era every road costs half the town's rate per
  metre to build (ADR 0036) and half its upkeep, so a village can afford to spread out. When
  the town arrives, upkeep of every road rises to the town rate: the paving bill, paid over
  time instead of at once. Undo refunds what an edit cost when it was made, so a road bought
  in the village refunds the village price. Roads still store no surface.
- **Buildings keep the era they were built in.** Each building records the city's era when
  it was put up, saved and hashed, and keeps it for life. The kit draws village buildings
  with a village `Style` and its cottage, small-shop and workshop designs, and town buildings
  as before. This is looks only: height, footprint, capacity and growth rules ignore the era.
  Replacing village buildings with town ones is #46's decision.
- **Snapshots.** The building record gains the era after `front`:
  `[zone, height, roof, front, era, n, x0, y0, …]`, 0 = village, 1 = town. Frame stats carry
  `era` and `townPopulation`. The build menu shows locked items with the residents still
  needed, and the tools refuse locked choices.
- **Saves and logs.** Save version 11 holds the era; older saves are refused (ADR 0033).
  Command logs keep their format; a `map` log replays as a village, a `grid` log as a town.
- **Later eras** append to `Era`. Each says what it unlocks in the sim's lock check, and the
  catalog marks the matching items.

## Consequences

- A new game cannot build apartments or main roads until it has grown a 1,500-person
  village of houses, small shops and workshops. Water (ADR 0052) matters only from then on,
  since only dense buildings need it. Fires (ADR 0053) start at 1,000, in the village.
  A balance check before the village discount: about 120 blocks of streets with low-density zones (three
  homes to each shop or workshop block) and one coal plant passed 1,500 in about three
  game days. One 8×8 grid alone levels off near 1,000, so a village has to spread out.
- Players see the town arrive at once on the roads; buildings change only as they are
  replaced, which #46 builds on.
- Tests that build main roads or dense zones on an empty map start it in the town era.
- A per-road era (so old roads stay dirt) would need road snapshot and save changes and a
  superseding ADR.
