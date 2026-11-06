# 0043. Residential lots mix standard and wider frontages

- Status: Accepted
- Date: 2026-11-05

## Context

Even spacious residential blocks repeat the same narrow frontage. ADR 0018's equal strips
leave long gardens but little variation along the street. Industry already combines free
neighbouring strips without disturbing existing lots (ADR 0019).

## Decision

Supersede ADR 0018's uniform frontage rule for new residential lots, at both densities.
Reuse the existing strip grouping: target one strip 60% of the time and two strips 40% of
the time, chosen from the city seed, road side and starting position. On a straight block,
this mixes roughly 16 m and 32 m frontages. Never target the four-strip industrial size.

Join only consecutive, unowned strips of the same residential zone on the same road side,
where their union is one polygon without holes. Mixed zones, curves and short frontages can
leave smaller lots. Commercial and industrial rules remain unchanged. Depth, alleys and house
footprint rules remain unchanged; wider house lots gain more side-yard space, not giant houses.

Reconstruct existing groups before choosing new ones, as in ADR 0019. Repainting and unchanged
recuts preserve existing lots and buildings, including those loaded from older saves. This
changes new layouts and replay results, not the save format or snapshot format.

## Consequences

- New residential streets have actual wider properties, not decoration crossing boundaries.
- Fewer low-density lots fit along a street, so it can house fewer households. Wider
  high-density lots can support larger apartment buildings under the existing footprint rules.
- Existing neighbourhoods do not gain variety automatically. Replacement lots after road
  edits use the new rules.
- Grouping costs apply during lot cutting, not ordinary simulation ticks. No new dependency.
