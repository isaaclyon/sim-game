# 0053. Fire stations send real trucks through traffic

- Status: Accepted; implemented
- Date: 2026-11-06
- Extends: [0021](0021-citizens-households-and-trips.md), [0026](0026-walking-trips-and-household-cars.md),
  [0036](0036-money-build-costs-taxes-upkeep.md), [0037](0037-pollution-and-land-value-steer-growth.md),
  [0042](0042-roadside-pieces-snap-to-roads.md), [0033](0033-single-city-state-saves.md)

## Context

M5 lists coverage services: fire, police, health and schools. The sim has no fires, crime or
sickness, and every citizen is an adult (ADR 0021), so schools need children first. The game
is built on a traffic sim, and services are a chance to make road layout and congestion
matter beyond commuting. The roadmap's sandbox "never fails", but the owner wants fire to
carry real stakes, with a way to turn it off. Project rules forbid code for future use, so
the shared shape for later services must be named without being built.

## Decision

- **First service: fire.** Police, health and schools follow as separate slices.
- **Fire station.** Placed by the player beside a road, snapping and facing it like coal
  (ADR 0042); its footprint clears zone cells and buildings. It has a build cost, upkeep per
  clock hour and a few jobs filled like any workplace, and placing it is refused when the
  city cannot pay. It owns a fixed number of trucks: its capacity.
- **Coverage area.** Buildings whose frontage junction lies within a fixed driving distance
  of the station's junction along roads (shortest path on the road graph, ignoring traffic).
  Recomputed when roads or stations change, not every tick. Covered buildings get a land
  value boost in the ADR 0037 land value update. A view shows covered roads and buildings.
- **Fires.** Only grown buildings burn; player-placed pieces never do. Each standing building
  has a small chance per fire check, drawn from a dedicated seeded RNG stream; industrial and
  high-density buildings are riskier. Fires do not spread. No fire starts while the resident
  population is below a threshold (about 1,000). A per-city setting, on by default, turns
  fires off; it is changed by a command, saved and hashed.
- **Response.** A new fire is assigned the free truck at the nearest station (by road
  distance) whose area covers it, if any. Trucks are agents of a new kind that drive the
  road network from the station to the building's frontage junction and back. They ignore
  signals and right of way at junctions but queue behind cars in lanes. A fire burns for a
  fixed time; a truck that arrives first puts it out and the building is saved. Otherwise
  the building is destroyed. A truck whose fire burns out before arrival turns back.
- **Destruction.** Households of a destroyed building move to a vacant home if one exists,
  else leave the city; its workers become unemployed and search as usual. The lot holds
  ruins for about a clock day, then may grow again under the normal growth gates. Ruins
  block growth and are removed by bulldozing early.
- **Display.** Flames and smoke on burning buildings and ruins on burnt lots are display
  only. Trucks are drawn as a new agent kind (kind 2 in the agent snapshot). A fire figure in
  the top bar shows the threshold warning as the city nears it, and turns red while a fire
  burns with no truck assigned; its panel lists those fires and can move the camera to one.
- **Undo.** Fire stations are placeables in the edit history of
  [0051](0051-road-edit-history-revised.md) once it lands. Fires and destruction are not undone.
- **Saves.** Stations, trucks, active fires, ruins, the fire RNG stream and the setting enter
  the save and state hash. The save version is bumped and older saves and links are refused
  (ADR 0033). Branches that bump the version in parallel (water, edit history) take the next
  number in merge order.
- **Seams for later services, not built now.** Police, health and schools are expected to
  reuse: a roadside station with cost, upkeep and jobs; a road-distance coverage area; a
  vehicle capacity; a land value boost; an overlay. The code is generalised when the second
  service arrives and proves which parts really are shared.

## Consequences

- Road layout matters for safety: a station behind a dead end or a jammed main road loses
  buildings. Traffic becomes a reason to plan, not only a cost of commuting.
- Sandbox can now lose buildings permanently, but only after the population threshold and
  only where the player has no or too little coverage; the setting gives a calm sandbox.
- A new agent kind changes the agent snapshot contract and the traffic model's junction
  rules; both are documented in `CLAUDE.md` when implemented.
- Every city saved before this change stops loading.
- Later: fire spread, staffing limits on trucks, police/health/schools on the same seams.
