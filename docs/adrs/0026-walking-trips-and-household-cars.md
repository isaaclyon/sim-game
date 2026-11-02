# 0026. Walking trips and household cars

- Status: Accepted
- Date: 2026-11-01

## Context

Every citizen drives every trip (ADR 0021). Real towns have adults without cars, and people
with cars walk short trips. Walking should take short trips off the road, make streets look
alive, and later let the player plan walkable neighbourhoods. ADR 0021 left walking to junction
access, and ADR 0022 expected pedestrians to need their own picking path. Pedestrian figures
were planned for M3; this pulls a simple version into M2.

## Decision

- **Cars belong to one adult.** When a household moves in, each member independently owns a
  car with a seeded chance (about 85%, a tuning value). A household never has more cars than
  adults. Others never borrow a car. A car is either parked at home or out with its owner.
  Cars are not drawn while parked and have no IDs of their own.
- **Mode is chosen once per round trip.** On leaving home, an owner drives if the first
  destination is farther than their walking limit, measured in a straight line. Otherwise they
  walk. The mode holds until they are home again, so a car always comes back.
- **Walking limits are per person.** Each citizen draws a walking limit and a walking speed at
  move-in from the resident RNG. Adults without a car accept longer walks. The ranges are
  tuning values.
- **Reach limits assignments.** Adults without a car take only jobs within their limit of
  home. Anyone on foot shops only within their limit of their current location. Car owners'
  job search is unchanged. A person with nothing in reach stays unemployed or skips shopping.
- **Walkers use the citizen's travel slot.** The stable `AgentId` from ADR 0021 now carries a
  walker or a car. A walking slot is in a new `Walking` state and steps through its own list
  each tick, outside lane queues. Walkers never block cars or each other.
- **Walk routes.** A walk runs from the origin building's front point along its frontage road,
  over the shortest-distance route on the road graph, to the destination's front point.
  Walk searches use plain lengths: no driver taste (ADR 0024), queue delay or main-road layer.
  Every road has sidewalks on both sides; walkers use the right-hand side of the lane they
  follow. Short walks between neighbours are visible; cars keep junction access.
- **Walkers obey signals; cars ignore walkers.** At a signalised junction, a walker continues
  only while the approach it arrives on has green, crossing with parallel traffic. Other
  junctions do not hold walkers. Junction reservations and conflict points are unchanged.
- **Failures and edits follow ADR 0021.** A walk with no route retries from the activity
  location. Road edits that remove part of a walker's route restart it from there. A car out
  at a demolished building returns home with its owner; relocating households keep their cars.
- **Agent snapshot stride grows to 5:** `[x, y, heading, speed_ratio, kind]` per agent slot,
  kind 0 = car, 1 = walker. This supersedes ADR 0003's agent format. The Rust writer,
  `protocol.ts` and the renderer change together, and walkers draw as small figures.
- **Inspection.** Walkers are picked by the same agent index as cars (ADR 0022). The citizen
  query reports whether the citizen is walking or driving.
- **Determinism.** Car ownership, cars out, walking limits, walking speeds and walker positions
  are hashed state. A completed-walk counter feeds an on-screen share of trips made on foot.

## Consequences

Supersedes ADR 0021's "access via junctions abstracts walking" and its vehicle-only slot, and
ADR 0022's separate pedestrian picking path. The rest of both stands.

Walkers cost a position update per tick each, much less than cars, and no pathfinding budget
beyond their own short searches. `traffic_bench` uses synthetic cars and does not measure
walkers; the LA target (ADR 0012) still counts cars, and walkers must fit in the non-traffic
half of the tick budget. The agent stream grows from 16 to 20 bytes per slot.

Spread-out towns raise unemployment among adults without cars, which is intended pressure.
Residential demand (ADR 0025) counts open jobs whether or not anyone can walk to them, so it
can keep asking for homes while those jobs stay out of reach.
There is no car sharing, buying or selling, parked-car rendering, sidewalk tool, crosswalk
conflict with cars, mid-block crossing, crowding, bikes, transit, children, or happiness
effect. Cars yielding to walkers would need an extension of ADR 0009.
