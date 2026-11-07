# 0036. Money: build costs, taxes and upkeep on an empty starting map

- Status: Accepted; implemented; free followed legs superseded by [0048](0048-drawing-over-roads-changes-their-class.md)
- Date: 2026-11-04
- Extends: [0033](0033-single-city-state-saves.md) (save version 2) and
  [0034](0034-command-log-replay-tests.md) (`map` start line)

## Context

M5 needs a reason to plan. Until now every road was free and a fresh city started on a
generated 60×60 street grid, about 700 km of road. Charging upkeep on that grid would bury a
village in costs it never chose. Sandbox mode (#41) and scenarios (#42) will later decide what
debt means; this decision only has to give them a balance that can run low.

## Decision

- **A fresh game city is an empty map** the size of the old grid (`World::empty`): no roads,
  the same zone grid extent. Every road on it is one the player or the sim built. The
  generated grid (`World::generate`) stays for tests and benchmarks. A command log names its
  start on the third line: `map cols rows` for the empty map, `grid cols rows` for the grid.
  The command log version does not change; older logs still read as grid logs.
- **The sim owns the money** in a treasury inside `World`: a balance in whole dollars (`i64`)
  and a carry of dollar-minutes below one dollar. Both are saved and hashed. Fresh cities
  start with $50,000.
- **Build costs:** `BuildRoad` and `BuildCurve` pay for the length of their new legs by class
  ($10/m street, $25/m main road, $5/m alley). Legs that follow an existing road cost nothing.
  A road whose cost exceeds the balance is refused like an invalid road and changes nothing.
  Bulldozing and zoning are free, and so are the alleys the sim cuts into deep blocks.
- **Upkeep and taxes** are rates per clock hour: each active road costs $0.10/m (street),
  $0.25/m (main) or $0.05/m (alley); each resident pays $2 and each filled job $1. The city
  settles every clock minute with integer arithmetic, carrying the remainder, so nothing is
  lost to rounding. Upkeep keeps charging below zero. Debt has no penalty except that it
  refuses new roads. The rates are tuning values, not part of this decision.
- **Quotes are read-only queries.** `road_cost` / `curve_cost` return what a build would cost,
  and `can_afford` whether the balance covers it. The road preview carries `cost` and
  `affordable`; preview requests carry the road class. The frontend shows the quote and
  the sim's balance, taxes and upkeep, and never computes money itself.
- Saves carry the treasury, so the save version becomes 2. Version-1 saves and links are
  refused with the existing unsupported-version error, as ADR 0033 requires; there is no
  conversion. Road upkeep per hour is derived from the roads and rebuilt on load.

## Consequences

The first session starts on bare land, so the first roads cost money and a sprawling network
can run the city into debt. Existing local saves and shared links stop opening; players must
start a New city. The grid replay fixtures reach the same population, buildings and trips;
only their hashes change. A generated grid owes about $100,000 an hour in upkeep, so grid
tests that build many roads give their world funds. New logs from the game start with `map`. Tax rates are fixed until happiness (#39) gives a reason to change them; a
player-set rate would add a command then.

## Amendment (2026-11-06)

Records what shipped after acceptance; the decision above stands.

- **A flat grant is a second income.** Every city receives $1,000 per clock hour (`GRANT`),
  whatever its size, settled each clock minute with taxes and upkeep. It pays for the first
  roads before there are residents to tax, and covers the upkeep of about 10 km of street.
  Frames carry it as `grant`. The amount is a tuning value. Adding it changed every city's
  balance, which is hashed, so the replay fixtures' `expect` hashes were re-blessed.
- **Free edits are always affordable.** `can_afford` accepts any cost of $0, so a city in debt
  can still move a road to a cheaper class (ADR 0048). Debt refuses only edits that cost money.
