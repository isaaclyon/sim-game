# 0045. The bulldozer demolishes buildings

- Status: Accepted
- Date: 2026-11-05
- Extends: [0013](0013-road-deletion-tombstones.md), [0023](0023-gradual-closure-of-rezoned-buildings.md), [0041](0041-power-grids-follow-roads.md)

## Context

The bulldozer removed plants, lines and roads, but not buildings. The only ways to clear a lot
were to repaint it (gradual closure, ADR 0023) or to cut it away with a road edit. A building
design depended only on the seed, lot and zone, so a lot that lost its building grew the same
building back.

## Decision

- `DeleteRoad` removes the plant under the point, else the nearest line in reach, else the
  standing building whose footprint holds the point, else the nearest road. Overlapping
  footprints keep the lowest building ID. The command keeps its name, so command logs and the
  worker protocol are unchanged.
- Demolition is immediate, like a road edit clearing a lot: households relocate or leave,
  workers lose their jobs, and trips to the building end. There is no closure step.
- Demolition is free (ADR 0036 keeps bulldozing free).
- The lot keeps its zone and is scheduled to regrow under the normal demand and power rules.
- A lot's replacement building, after demolition or any other removal on a lot that stays,
  draws its design from the per-lot stream mixed with the ID of the building it replaces.
  A lot's first building is unchanged. The building store keeps the last building ID per lot
  for this; the save layout does not change.
- The bulldoze preview outlines the building's footprint as a closed `[x, y]` ring, as it does
  for a plant.

## Consequences

Players can clear single buildings and reroll a lot's design. Demolition is not part of the
road edit history (ADR 0039); a demolished building stays gone. Lots regrowing after gradual
closure also get fresh designs; the existing replay fixtures were unaffected. Picking scans standing footprints, which is cheap next to traffic at current city sizes.
