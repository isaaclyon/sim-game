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

**Trip**:
A citizen's journey from one place to another for an activity, such as work, shopping or returning home.
_Avoid_: route (when you mean the journey rather than its path)

**Citizen inspector**:
The panel that shows one selected citizen's name, home, job and current trip, and can make the camera follow their car.
_Avoid_: car inspector, agent inspector

## Zoning

**Zone**:
What the player allows to be built on a piece of land: residential, commercial or industrial,
each at low or high density, or none.
_Avoid_: land use, zone type (when you mean the full zone including density)

**Density**:
The low or high level of a zone, chosen by the player: houses vs. apartments, corner shops vs.
stores. It never changes on its own.
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
