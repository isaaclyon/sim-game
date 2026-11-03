# 0032. Compact shops support small teams

- Status: Accepted
- Date: 2026-11-03
- Supersedes: low-density commercial staffing rate in [0030](0030-small-workplace-capacity.md) only

## Context

ADR 0030's one job per 100 m² rate reduces oversized full-lot shops to modest workplaces.
ADR 0031 then replaces those deep footprints with compact neighbourhood shops. Applying the
same staffing rate to these much smaller trading floors leaves almost every shop at the
two-job minimum, even when it looks large enough for a small team.

## Decision

- Low-density commercial capacity is `max(2, floor(footprint area / 25))`. A 200 m² shop
  supports eight jobs; 130 m² supports five and 256 m² supports ten.
- Count ground-floor space only. Upstairs, yards and planted space do not add jobs. Capacity
  is total assigned employment, not the number of people physically present at once.
- Keep the small-shop footprints, heights and visual style from ADR 0031. High-density
  commerce, industry, household capacities and demand shares stay unchanged.

## Consequences

The inspector and employment allocation read the same sim-owned capacity. Across eight
fully developed 120 m blocks and eight seeds, compact shops provide 605 jobs for the
commercial share of 275 house residents: one commercial block supports about 5.5 house
blocks. The land-balance test bounds this ratio to catch both under- and overstaffing.

Capacity is assigned when a building registers. Running worlds retain their existing
capacities; reloading starts a new city using this rate. Replays can grow differently but
remain deterministic. No snapshot or protocol change is needed.
