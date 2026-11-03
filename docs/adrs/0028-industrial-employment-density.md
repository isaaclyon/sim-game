# 0028. Industrial employment density

- Status: Accepted; industrial minimum job counts superseded by [0030](0030-small-workplace-capacity.md)
- Date: 2026-11-02
- Supersedes: industrial job-capacity tuning in [0021](0021-citizens-households-and-trips.md) only

## Context

The original rule gives both industrial densities one job per 60 m² of footprint. A
high-density industrial zone therefore adds no employment intensity. Dense housing creates
far more demand than these halls can serve, making industry dominate the city's land area.

Across eight seeds on fully developed 120 m square lattice blocks, houses admit 275 residents
and apartments admit 13,835 residents in total. Eight industrial blocks supply 505 jobs at
either density. At the unchanged 60% industrial job share, one apartment block needs about
16.4 industrial blocks. These are land-capacity comparisons, not a prediction of employment
or a growth timetable; the measurement disables demand gating to fill each block.

## Decision

- Low-density industry provides one job per 30 m² of building footprint; high-density
  industry provides one per 10 m². Round down, with a minimum of one job per building.
- Density means employment intensity, not additional floors inferred from hall height.
  Footprints, lots, height, household sizes and commercial capacity stay unchanged.
- Keep the 40% commercial / 60% industrial demand split and starting allowances from ADR 0025.
  Changing that split alone would move the land requirement to shops rather than address
  industrial capacity.

## Consequences

In the same eight-seed comparison, low industry supplies 1,020 jobs and high industry 3,114.
One low industrial block covers the industrial share for about 6.2 house blocks; high industry
covers about 18.9. One apartment block needs about 2.7 high industrial blocks instead of 16.4.
The test guards those land-balance outcomes as well as the density distinction.

These remain gameplay tuning values, not a claim about real-world factory staffing. The
high-density value trades lower realism for a less industrial-dominated town without changing
housing or introducing another employment sector. Larger job additions can overshoot demand;
existing demand gating already stops subsequent buildings. Old command logs produce different
capacity and growth under this version. No snapshot layout changes, and no extra per-tick scan.
