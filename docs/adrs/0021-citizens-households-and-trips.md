# 0021. Citizens, households, homes, jobs and trips

- Status: Accepted
- Date: 2026-11-01

## Context

Population must count residents, not moving cars. Homes can hold households of different
sizes, and most residents are idle. Road edits and failed routes must not count as arrival.
The approved M2 design separates residency, employment and activities from traffic.

## Decision

- Citizens, households and homes have append-only IDs and struct-of-arrays storage. All
  citizens are adults. Household membership is fixed; members share one home and relocate
  together. A house has one home; apartments have several independently occupied homes.
- Capacity is separate from occupancy. Usable residential area is footprint area times
  `floor(height / 3)`. Houses hold `floor(area / 25)`, clamped to 2–7. Apartments provide
  `max(1, floor(area / 80))` homes, each holding `floor(area / homes / 25)` clamped to 2–7.
  Commercial jobs use one position per 30 m² across `floor(height / 4)` floors; industry
  uses one per 60 m² of footprint only (a tall hall is not several floors). Minimum one job.
  These are tuning constants, not a configuration system.
- Empty completed homes admit households after seeded delays of 5–45 sim seconds, without
  requiring jobs or migration vehicles. Sizes 1–7 have weights 25/35/18/12/6/3/1; exclude
  sizes beyond capacity and renormalize. Building growth remains the existing timed system.
- Unemployed adults search every 120 sim seconds for reachable vacancies with seeded distance
  preference. A cached connected-component map filters eligible destinations; it refreshes
  on road-version changes, including tombstones and alleys. This relies on today's two-way
  roads, not on a claim that connectivity replaces actual trip routing or failure handling.
  Jobs persist across temporary disconnection. Commercial buildings alone offer shopping.
  Morning departures and work durations vary per person. Work/shopping duration starts on
  arrival. Shopping is optional, including daytime shopping for unemployed residents.
  New residents have a first activity within minutes; subsequent days use the existing
  four-real-hour day clock. Event heaps wake residents; idle citizens are not scanned per tick.
- A trip records an intended activity and destination, retaining the last valid activity
  location until arrival. Access uses a junction of the building's frontage road. Same-junction
  trips arrive immediately without a vehicle or path failure. Failed routes stay at the
  activity location and retry every 30 sim seconds. Invalidated and stuck trips recover there,
  not to their destination. Missing activities are skipped; trips never overlap.
- Each citizen lazily reserves one stable AgentId on their first nontrivial trip. The slot is
  reused only by that citizen, hidden and dormant between trips. Departed citizens' slots
  are never reassigned. Synthetic SpawnAgents retain their independent random-trip lifecycle.
  This completes ADR 0005's agent removal contract without recycling identity. It supersedes
  ADR 0006's lifecycle/random-trip and stuck recovery rules for citizen-owned agents only,
  and ADR 0013's lane-end recovery rule for citizen-owned agents only.
- Building original use remains unchanged on repaint. Assignment eligibility is separate:
  original zone must equal the current lot zone, and building/lot/access must remain valid.
  Closing buildings accept no new household, job or shopping assignment. This does not
  implement gradual closure (#17). Existing residents and employees remain until demolition.
  Exact original-zone restoration immediately restores eligibility.
- After demolition, release jobs; relocate each whole displaced household to an eligible
  vacant home of sufficient capacity or mark its members departed. Cancel affected vehicles
  and recover removed activity locations to the household's home. Removed destinations never
  produce arrivals. Building removal and occupancy reconciliation are explicit seams for #17.
- Population is authoritative sim state exposed through WASM and worker statistics. Normal
  startup has no synthetic cars. Benchmark spawning remains explicit and unchanged.

## Consequences

Stable IDs preserve snapshot interpolation and future inspection. Storage and snapshot/stat
scans grow with citizens who have ever travelled, not simultaneous trips; departures leave
permanent slots. Snapshots still write 16 bytes per allocated vehicle slot after everyone
leaves. This is a deliberate memory tradeoff, not an LA-scale solution. A future
packed snapshot/handle scheme needs its own contract. No idle per-tick citizen scan is added.

Routing and activity timing remain deterministic and included in state hashing. Access via
junctions abstracts driveways, parking and walking. There are no children, relationships,
aging, stock, visitor limits, money, demand or happiness penalties in this change.
