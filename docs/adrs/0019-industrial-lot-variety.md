# 0019. Industrial lots combine neighbouring frontage strips

- Status: Industrial footprint fallback superseded by [0020](0020-rectangular-industrial-halls.md); otherwise accepted
- Date: 2026-11-01

## Context

The first buildings make industrial districts look like rows of identical narrow sheds.
The player wants small workshops, wider warehouses and occasional factory-sized sites,
without rebuilding existing neighbourhoods. ADR 0018 gives every use roughly 16 m frontages.
Changing that division changes replay results, so it is part of the lot contract.

## Decision

Supersede ADR 0018's uniform frontage rule for **new industrial lots only**. Start with its
existing road-following strips and sliver cleanup. Combine consecutive strips on the same
road side into seeded groups targeting one, two or four strips. Smaller sites remain common;
four-strip factories are occasional. A join requires one connected polygon without holes,
and every cell in both strips must have the same industrial zone and no existing owner.
Mixed-zone edges and insufficient space keep smaller lots. Reach and alley rules do not change.

On recuts, first reconstruct groups belonging to the same existing lot, regardless of its
current zone. The existing road, cells, front point and outline matching rules then determine
whether its ID survives. Retire unmatched lots before cutting their replacement sites, so
new sites follow the current zone instead of inheriting retired owners' grouping. Zoning
alone never combines or subdivides existing lots. Road edits still remove the buildings on
retired lots. Lot IDs remain append-only.

Industrial buildings remain one hall per lot. Use road-aligned boxes with varied proportions
and yard space, falling back to an inset lot shape where a box cannot fit. Frontage sets the
scale: workshops on small sites, warehouses on medium sites and factories on larger ones.
Seeded height and roof choices add variation without changing traffic randomness. These are
visual descriptions, not new economic building types. Existing buildings keep their geometry,
height, original use and roof choice when zoning changes.

Building snapshots become `[zone, height, roof, n, x0, y0, …]`. `roof` is a sim-owned palette
index (0–3 for industry, 0 for other uses). It is stored with the building and included in the
state hash. The renderer maps it to colour, but does not choose new random state.

## Consequences

- Industrial sites have real larger lot boundaries, not buildings covering neighbours.
- No land crosses zone boundaries merely to make a factory, and repainting cannot erase homes.
- Curves and small blocks may prevent large sites; a full block is not guaranteed a factory.
- Replayed worlds cut different industrial lots under this version. There is no save format yet.
- Polygon joins cost work during lot edits, not on idle simulation ticks. No new dependency.
- Non-industrial lot sizes and building shapes stay unchanged. Detailed roofs, machinery,
  multi-building compounds, demand and redevelopment remain outside this change.
