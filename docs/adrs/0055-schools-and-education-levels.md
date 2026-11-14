# 0055. Schools teach children, and education shapes who fills jobs

- Status: Accepted; amended 2026-11-09 and 2026-11-13; slices 1 and 2 implemented; extended by
  [0056](0056-taxes-follow-city-quality-and-services-have-budgets.md) (seat blocks and budgets)
- Date: 2026-11-06
- Extends: [0054](0054-children-births-and-growing-up.md), [0021](0021-citizens-households-and-trips.md),
  [0026](0026-walking-trips-and-household-cars.md), [0036](0036-money-build-costs-taxes-upkeep.md),
  [0037](0037-pollution-and-land-value-steer-growth.md), [0042](0042-roadside-pieces-snap-to-roads.md)
- Follows the service seams named by ADR 0053 (fire)

## Context

With children (ADR 0054), schools can matter. The owner wants education to shape the future
workforce rather than to push families out, with several levels, and with university
arriving only when offices give graduates something to do (M6 town era).

## Decision

- **Levels.** Every citizen has an education level: none, primary, high school or
  university. Adults moving in arrive with a seeded mix: mostly primary, some high school, a
  few university. Babies start with none.
- **Schools.** This slice builds primary schools and high schools; universities come with
  offices in M6. Each is placed by the player beside a road (ADR 0042), clears its land,
  has a build cost, upkeep per clock hour and teacher jobs, is refused when unaffordable,
  and is a placeable in the edit history (ADR 0051). Each has a fixed number of seats and a
  coverage area: a driving distance along roads, as for fire stations. Homes inside a
  school's area get a land value boost.
- **Stages.** Primary covers the first half of childhood, high school the second half.
- **Enrolment.** At the start of each stage, and when seats or roads change, an unenrolled
  child takes a seat at the nearest school of their stage, by road distance, whose area
  covers their home and which has a free seat. Ties break by school ID.
- **School days.** Enrolled children travel home → school → home on the day schedule. Short
  trips are walked; longer ones are driven by a household adult with a car, under ADR
  0026's car rules (the adult's own activities shift around it). Children are drawn as
  smaller walkers.
- **Graduating.** A child who stays enrolled through a stage gains that level. A child with no
  seat stays home and grows up with a lower level. Nobody leaves over schooling.
- **Jobs prefer education.** Low-density jobs need none. Dense industry prefers primary,
  dense commerce prefers high school, teacher jobs prefer high school; university fills any
  job. A vacancy is offered first to job seekers with at least its level; a less-educated
  seeker can take it only after a seeded delay, so such jobs fill more slowly but never stay
  empty for lack of schools alone.
- **Display.** The inspector shows a citizen's education and school. A schools view shows
  coverage and seat use; the top bar's population panel shows education shares and
  unschooled children.
- **Saves.** Education levels, enrolment, schools and their seats enter the save and state
  hash. The save version is bumped and older saves are refused (ADR 0033).

## Consequences

- Dense commerce grows faster in towns that built schools a childhood earlier; the payoff
  of a school is delayed, which rewards planning.
- School runs add morning and afternoon traffic and sidewalk crowds.
- Schools are the second coverage service after fire; code shared with fire (roadside
  station, road-distance area, capacity, land value boost) is generalised here, as ADR
  0053 planned.
- University, offices and old age remain later work.

## Amendment (2026-11-09): schools are buildings, teachers are real, walking first

Found while scoping: service jobs (fire stations, coal plants) only raise demand, and trips,
jobs and locations can only point at buildings. Where this section and the text above
disagree, this section wins.

- **Schools are buildings.** A placed school is a building of a new service kind in the
  building store, so trips, jobs, inspection and frontage work unchanged. Growth, closure,
  zoning and fire ignore it; it is removed only by bulldozing.
- **Teachers are real.** Teacher jobs are job positions citizens fill and commute to, offered
  first to high-school-educated seekers like other jobs. Seats and teacher jobs come in seat
  blocks (ADR 0056).
- **Walking first.** Until a later slice adds adult driving, enrolled children walk, and a
  school is open to a child only if it is within the child's walking reach as well as its
  area. The driving rule above then applies unchanged.
- **Slices.** 1) education levels, the arrival mix and jobs preferring education; 2) placeable
  schools, seat blocks, coverage, enrolment, graduating and teacher jobs; 3) school runs and
  child walkers; 4) the schools view, inspector and top bar; 5) adult driving.


## Amendment (2026-11-13): slice 2 rules

Settled while building slice 2. Where this section and the text above disagree, this section
wins.

- **Kinds.** A primary school has a 28 m site, costs $4,000, and per seat block has 30 seats,
  2 teacher jobs and $20 an hour of upkeep; its area reaches 800 m along roads. A high school
  has a 36 m site, costs $7,000, and per block has 40 seats, 3 teacher jobs and $35 an hour;
  its area reaches 1,200 m. Every school opens with 3 seat blocks until ADR 0056 adds budgets.
- **No utilities, any era.** A school needs no power or water, never burns, and can be placed
  in a village. Its teacher jobs count as commercial jobs in demand.
- **Graduating.** "Stays enrolled through a stage" means: the child holds a seat when the
  stage ends and was enrolled for at least half of it. A child who arrives in town part-way
  through a stage counts as schooled elsewhere for the part already behind them. A child who
  has no seat when the stage ends gains nothing, whatever came before.
- **Seats are kept.** A seated child keeps their seat when roads change, even if the school
  no longer covers their home. Only children without a seat look again, when a school is
  placed or removed or roads change, in citizen ID order. A child who moves home within the
  city gives up their seat and looks again; the time already spent enrolled still counts.
- **Bulldozing** a school frees its seats and ends its teachers' jobs; its pupils look for
  another school at once. Undo puts the school back empty.
- **Land value.** Cells of homes inside any school's area gain 10% of full land value, once,
  however many schools cover them.
- **Display.** `schools` carries each school's site for drawing; the building kit skips school
  buildings (zones 7 and 8), which have their own models. Clicking a school selects nothing
  until slice 4.
- **Saves.** Save version 12. A city that never had a school hashes as it did before.
