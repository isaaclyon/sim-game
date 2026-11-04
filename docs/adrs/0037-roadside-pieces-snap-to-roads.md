# 0037. Roadside pieces snap to the road and face it

- Status: Accepted
- Date: 2026-11-04
- Extends: [0036](0036-power-grids-follow-roads.md)

## Context

ADR 0036 placed every plant as an axis-aligned square centred on the cursor. Coal had to
touch a road, so the player had to find, by trial, a spot close enough to the road but clear
of it, and on any road that is not axis-aligned the plant sat at an angle to its street.
Line ends joined a plant anywhere inside it and a road at its centreline, so poles stood in
the middle of buildings and streets. Service buildings, stations and other pieces still to
come will also stand on a street and face it, so this needs one rule rather than one per tool.

## Decision

- **Frontage.** `sim::frontage` finds the road centreline nearest a point within a reach, the
  side of the road the point is on, and the unit direction from there toward the road. A
  road's *kerb* is the outer edge of its sidewalk: 8 m from the centreline for main roads,
  6 m for streets and alleys, matching the frontend's widths.
- **Coal snaps.** A coal plant asked for within 80 m of a road stands on the cursor's side of
  the nearest one, its front edge 1 m outside the kerb, turned to face the road, and slides
  along it with the cursor. Farther from any road it stays where asked and is refused, as
  before. Solar and wind stand where asked, facing +y. The command still carries the cursor;
  the sim does the snap, so previews, commands and replays agree.
- **Plants store a front.** Each plant stores the unit direction it faces, beside its centre.
  The footprint is a square turned to that direction. Everything that touched a plant's
  rectangle now works in the footprint's own frame, so no trigonometry enters sim state.
- **Terminals.** Each plant has one terminal, 2 m behind the middle of its back edge, away
  from its road. A line end inside a plant, or within line reach of it, snaps to the terminal.
- **Roadside poles.** A line end within snap reach of a road stands on that road's sidewalk,
  1 m inside the kerb, on the side facing the line's other end, so a line never crosses the
  road it joins. Line ends still prefer an existing line end first.
- Plant fronts are saved and hashed. The save format changes on the same unreleased branch as
  ADR 0036, so the version bump there covers it.

## Consequences

Coal is placed in one move along any street, at any angle. Lines run from a plant's back to a
pole on the sidewalk. Future roadside pieces reuse `frontage` and the kerb rule; any that are
not square will need a footprint with separate width and depth. A plant snapped near a
junction can still overlap the crossing road and is refused; sliding it clear is not done.
The plant snapshot gains the front and terminal (`PLANT_STRIDE` 10).
