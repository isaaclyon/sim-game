# 0054. Children are citizens who are born, grow up and move out

- Status: Accepted; not yet implemented
- Date: 2026-11-06
- Supersedes: "All citizens are adults" and "Household membership is fixed" in
  [0021](0021-citizens-households-and-trips.md)
- Extends: [0026](0026-walking-trips-and-household-cars.md), [0022](0022-citizen-inspection-query.md),
  [0033](0033-single-city-state-saves.md)

## Context

Schools (ADR 0055) need children, and the sim has none: every citizen is an adult and
households never change (ADR 0021). The owner wants children to be visible people, to grow
up into the city's workforce and to make the population grow from within, without adding old
age yet. One clock day is about 80 real minutes at 1×.

## Decision

- **Children are citizens** with the same append-only IDs and struct-of-arrays storage,
  plus a birth tick. A citizen is a child until their age reaches about 10 clock days (a
  tuning value). Children never work, shop or own a car. In this slice they stay home; school
  trips arrive with ADR 0055.
- **Arrivals.** Household sizes keep ADR 0021's weights and home-capacity limits. A seeded
  share of arriving members beyond the first adult are children, with seeded ages.
- **Births.** A household with at least one adult whose home has room (size below the home's
  capacity) may have a baby, with a small seeded chance per check from a dedicated RNG
  stream. No birth happens in a full home.
- **Growing up.** A child who comes of age becomes an adult at once, looks for work like any
  unemployed resident (ADR 0021) and keeps living with the household. Each adult who grew up
  here wants their own home: when a vacant home is available they move into it as a new
  household, ahead of new arrivals for that home. If none appears within a few clock days
  (tuning) they leave the city. Their car, if any, follows ADR 0026's move-in rules.
- **Household membership** therefore changes by birth, a grown child moving out, and leaving
  the city. Households stay in one home; ADR 0021's relocation rules are otherwise unchanged.
- **No old age.** No retirement or deaths in this slice. Growth stays bounded by housing,
  because births need room and grown children need homes.
- **Population** counts children. Statistics and the HUD show adults and children
  separately where they matter (workforce, unemployment use adults only).
- **Inspection** (ADR 0022) shows each citizen's age and, for children, that they live at
  home. Names stay derived from the seed and ID. Children are drawn as smaller walkers when
  they travel (from ADR 0055 on).
- **Saves.** Birth ticks, the birth RNG stream and moving-out deadlines enter the save and
  state hash. The save version is bumped and older saves are refused (ADR 0033).

## Consequences

- Cities grow from within: a town with spare housing gains residents without new arrivals,
  and a full town pushes its grown children out.
- Every household-size assumption (unemployment rates, power and water demand by homes,
  fire rehoming) must count only the right citizens; children have no job and no car.
- Replays change once, because arrivals now include children; expectations are re-blessed.
- Old age, retirement and deaths remain later work.
