# 0030. Small-workplace employment capacity

- Status: Low-density commercial staffing rate superseded by [0032](0032-compact-shop-staffing.md); otherwise accepted
- Date: 2026-11-02
- Supersedes: low-density commercial capacity in [0021](0021-citizens-households-and-trips.md)
  and industrial minimum job counts in [0028](0028-industrial-employment-density.md) only

## Context

Building inspection exposes a mismatch at the small end: a two-storey low-density shop can
report 54 jobs, while a tiny industrial building with a service yard reports just one. The
shop formula treats upper floors as equally staff-intensive trading space. The industrial
area formula needs a minimum team size without inflating the capacity of large halls again.

## Decision

- Low-density commercial buildings provide `max(2, floor(footprint area / 100))` jobs. Count
  ground-floor trading space only; extra visual height does not add jobs. Upper floors do
  not become residential homes or a separately simulated use.
- High-density commercial buildings retain one job per 30 m² across `floor(height / 4)`
  floors, with a minimum of one. This change is not a reduction in dense commercial capacity.
- Industry retains one job per 30 m² of footprint at low density and per 10 m² at high density,
  with minimums of five and ten jobs respectively. Hall height and yard area do not add jobs.
- Keep demand shares, starting allowances, footprints and household capacities unchanged.
  These values remain sim-owned gameplay tuning, not a staffing estimate for real businesses.

## Consequences

An 810 m², two-storey low shop goes from 54 jobs to eight. Large factories retain the recent
employment-density increase; only small sites gain jobs from the minimums. Low commercial
land grows more buildings before satisfying demand. Across eight fully developed 120 m lattice
blocks and eight seeds, shops supply 896 jobs for the commercial share of 275 house residents,
so one shop block still supports about 8.1 house blocks. Industrial supply becomes 1,074 low
or 3,177 high jobs across those eight blocks; an apartment block needs about 2.6 high industrial
blocks. These compare land capacity with demand gating disabled, not a growth timetable.

Capacity is assigned at building registration. Reloading the app starts a city with the new
rules; already running worlds retain the capacities they registered. Replays under the new
version may grow differently, but remain deterministic. No inspector or snapshot format changes.
