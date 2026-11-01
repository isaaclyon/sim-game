# sim-game

A city builder where the player lays roads and paints zones, and a deterministic simulation
grows and runs the city.

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
