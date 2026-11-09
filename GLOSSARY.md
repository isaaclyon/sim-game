# sim-game

A city builder where the player lays roads and paints zones, and a deterministic simulation
grows and runs the city.

## Citizens

**Citizen**:
An individual resident of the city, whether at home, at another destination or travelling.
_Avoid_: agent, car (when you mean the person)

**Household**:
One or more citizens who share a home and move in or relocate together. Members can have different workplaces and daily activities.
_Avoid_: family (when no family relationships are implied)

**Home**:
A dwelling that accommodates one household. A house provides one home; an apartment building can provide several.
_Avoid_: building (when you mean an individual dwelling)

**Home capacity**:
The maximum number of residents a home accommodates, distinct from how many currently live there.
_Avoid_: population, occupancy

**Population**:
The number of citizens who live in the city, including those who are not travelling.
_Avoid_: vehicle count, agent count

**Education**:
The highest school level a citizen has finished: none, primary, high school or university.
Dense jobs prefer more of it and fill more slowly without it (ADR 0055).
_Avoid_: skill, qualification

**Trip**:
A citizen's journey from one place to another for an activity, such as work, shopping or returning home.
_Avoid_: route (when you mean the journey rather than its path)

**Citizen inspector**:
The panel that shows one selected citizen's name, home, job and current trip, and can make the camera follow their car.
_Avoid_: car inspector, agent inspector

**Mood**:
How content one citizen is, 0 to 100, derived from their home's services and pollution, their
job and their commute. Each thing that lowers it is a complaint (ADR 0058).
_Avoid_: satisfaction, approval

**Happiness**:
The city's mood: residents' average. A happy city draws new residents; a household that stays
miserable for a clock day may leave (ADR 0058).
_Avoid_: land value (how good a place is to build, not how people feel)

**Commute**:
How many clock minutes a worker's last trip to work took, waiting included.
_Avoid_: travel time (when you mean any trip)

## Zoning

**Zone**:
What the player allows to be built on a piece of land: residential, commercial or industrial,
each at low or high density, or none.
_Avoid_: land use, zone type (when you mean the full zone including density)

**Density**:
The low or high level of a zone, chosen by the player: houses vs. apartments, corner shops vs.
stores. It never changes on its own. High density waits for the town era (ADR 0059).
_Avoid_: level, tier

**Zone cell**:
One square of the zone grid, the smallest piece of land the player can zone.
_Avoid_: tile, pixel

**Zone grid**:
The fixed grid of zone cells that covers the whole map. It records what the player painted,
independent of roads and blocks.
_Avoid_: zone map, zoning layer

**Zone brush**:
The tool that sets every zone cell inside a dragged rectangle to one zone.
_Avoid_: zone tool, painter

**Unzone**:
Painting with no zone, which returns zone cells to unzoned land.
_Avoid_: erase, dezone

**Closure**:
How a building empties out after its lot is repainted to a different zone or density, or
unzoned. Its workers lose their jobs, its households move or leave one at a time, and the
empty building is then removed so the lot's current zone can grow. Painting the original
zone back cancels it (ADR 0023).
_Avoid_: demolition (road edits remove buildings at once), eviction

**Demand**:
How many more people the town wants housed (residential), or wants as shop or industry jobs
(commercial, industrial). People move in where there are jobs, and businesses open where
there are people. A zoned lot builds only when demand exceeds its land-value threshold and
there is enough power; otherwise it waits.
_Avoid_: need, desirability

## Power

**Power plant**:
A player-placed structure that supplies power: coal (steady, needs a road and workers), solar
(daylight only) or wind (changes with the wind).
_Avoid_: generator, power station

**Transmission line**:
A line of poles the player draws to carry power where roads don't, such as from an off-road
plant or between road networks. Nothing can be built on the land it crosses.
_Avoid_: power line (when you mean the roads that carry power), cable

**Frontage**:
Where something placed beside a road meets it: the nearest road, and which way to face it.
Coal plants snap to their frontage; other roadside pieces will too.
_Avoid_: lot frontage (the width of a lot along its road)

**Kerb**:
The outer edge of a road's sidewalk. Roadside pieces stand just outside it; line poles that
join a road stand just inside it.

**Terminal**:
The one point on a plant where transmission lines join it, behind its back edge.
_Avoid_: socket, port

**Grid**:
Everything connected for power: roads joined by road or transmission line, and the plants
and buildings on them. Each grid shares its own supply.
_Avoid_: network (that is the road network), zone grid

**Supply**:
The power a grid's plants produce right now.
_Avoid_: capacity (when output varies)

**Power demand**:
The power a building needs, set by its homes and jobs.
_Avoid_: demand (that is R/C/I demand), load

**Powered**:
A building its grid has enough supply for. Older buildings are served first. An unpowered
building keeps its people but fills no vacancies, and an unpowered lot does not build.
_Avoid_: online, connected (a building can be connected and still unpowered)

**Starter power**:
A small free supply for the whole city, enough for a hamlet, so a village can grow before
its first plant.
_Avoid_: base supply, free power

## Money

**Balance**:
The city's money in whole dollars. Building roads spends it; taxes and upkeep change it every
clock minute. It can fall below zero (ADR 0036).
_Avoid_: funds (except the starting funds), cash

**Build cost**:
What a new road costs, by length and road class, paid from the balance when it is built. A
village pays half (ADR 0059). A road that costs more than the balance is refused. Bulldozing, zoning, sim-built alleys,
power plants and transmission lines are free.
_Avoid_: price

**Upkeep**:
What every road costs per clock hour to keep, by length and class. Half in a village; the
town's arrival raises it to the full rate (ADR 0059).
_Avoid_: maintenance (a later service), running cost

**Taxes**:
What residents and filled jobs pay per clock hour, at fixed rates.
_Avoid_: income (taxes minus upkeep is the net change)

**Service budget**:
How many units a service building runs: trucks for a fire station, seat blocks for a school.
Upkeep and staff jobs are per unit (ADR 0056).
_Avoid_: funding, spending (when you mean one building's units)

**Tax rate**:
A player-set level for residential, commercial or industry taxes; higher rates raise money
and cost happiness (ADR 0056, not yet built).

**Debt**:
A balance below zero. Upkeep keeps charging; no new roads can be built until taxes bring the
balance back.
_Avoid_: bankruptcy (scenarios may define it later)

## Pollution and land value

**Pollution**:
Harm that industry and traffic do to the land around them, in three kinds: air pollution,
ground pollution and noise. It lowers land value (ADR 0037).
_Avoid_: smog (when you mean all kinds), dirt

**Air pollution**:
Pollution carried through the air from factories and busy roads, spreading evenly over a wide
area. It follows current sources and is gone when they are.
_Avoid_: smog (in code and docs), emissions

**Ground pollution**:
Contaminated land under and right beside factories. It builds up while a factory runs and
fades slowly after it closes, so old factory sites stay tainted for a while.
_Avoid_: soil pollution, contamination

**Noise**:
Pollution heard near busy roads and factories, strongest right beside the source. It follows
current sources. Not the sound effects the game plays, and not route noise (ADR 0024).
_Avoid_: noise pollution (just say noise), sound

**Traffic flow**:
How many cars use a road, averaged over time so it changes slowly. It decides how much air
pollution and noise a road makes.
_Avoid_: traffic (when you mean the cars on the road now), congestion

**Land value**:
How good a place is to build on. Pollution lowers it; later, services and parks raise it. Homes
care most, shops a little and industry not at all. Low land value delays growth but never
empties a standing building.
_Avoid_: desirability, price (land is not bought or sold), happiness

## Saving and sharing

**Save**:
The complete state of one city as validated bytes, kept in one local slot per browser (ADR 0033).
_Avoid_: snapshot (that is what the renderer draws), replay

**Shared city**:
A save sent as a `.city` file or a `#city=` link. Opening one forks it: the receiver gets an
independent copy that replaces their local city, and later edits never reach the sender (ADR 0035).
_Avoid_: sync, multiplayer, import (when you mean opening a friend's city)

**Command log**:
The seed, map size (an empty map, or the generated street grid) and every player command with its tick, from a fresh start. Replaying it
rebuilds the same city on the same build and target; saved logs are replay regression tests
(ADR 0034).
_Avoid_: save, history, undo log

## Eras

**Era**:
The city's stage of growth, sim state that only moves forward: village, then town. Population
unlocks the next era, which unlocks tools (ADR 0059).
_Avoid_: age, level, tier

**Village**:
The first era. Streets are drawn as dirt tracks; main roads and dense zones are locked.
_Avoid_: hamlet

**Town**:
The era a village reaches at 1,500 residents. Roads are paved and main roads and dense zones
unlock. "Town" also stays fine as a loose word for the player's city.

**Building era**:
The city's era when a building went up, kept for life. It picks the building's look, nothing
else (ADR 0059).
_Avoid_: style (that is the kit's palette)
