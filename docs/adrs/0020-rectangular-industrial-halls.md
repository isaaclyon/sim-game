# 0020. Industrial halls stay rectangular

- Status: Accepted
- Date: 2026-11-01

## Context

ADR 0019 falls back to an inset lot polygon when a rectangular industrial hall cannot fit.
In the local preview this makes long triangular sheds and pointed buildings at junctions.
The player wants industrial buildings to look like halls, not copies of their lot outlines.

## Decision

Supersede only ADR 0019's industrial footprint fallback. Keep the existing road-aligned box
fitting, including smaller boxes and nearby positions. If none fits with the required yard
clearance and minimum area, leave the lot open. Do not use the lot polygon as a building.

Industrial lot outlines may still be wedges or other irregular shapes. Lot grouping, seeded
size and colour variation, snapshots, and preservation of existing buildings do not change.
Residential and commercial footprint rules do not change.

## Consequences

- New industrial buildings have rectangular footprints, including at curves and junctions.
- More awkward lots remain open. Filling every lot is less important than plausible halls.
- Existing buildings are not rebuilt. Replays under this version omit the former fallback sheds.
