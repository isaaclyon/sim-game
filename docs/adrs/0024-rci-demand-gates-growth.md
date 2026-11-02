# 0024. R/C/I demand gates growth

- Status: Accepted
- Date: 2026-11-03

## Context

Buildings grew on every zoned lot after a seeded delay, whatever the town needed (ADR 0016,
ADR 0023 left replacement growth on that timer). ROADMAP M2 asks for a simple R/C/I balance
that decides what gets built next. People should move in where there are jobs, and businesses
should open where there are people. Growth must stay deterministic and add no idle per-tick
scans.

## Decision

- Demand is computed in people from authoritative sim state, per use (density is the
  player's choice and shares its use's demand):
  - Residential = 40 + open jobs − residents − 2 × vacant homes. Open jobs minus residents is
    job vacancies minus unemployment; vacant homes count the households still on their way,
    so one tick of growth does not fill the same demand twice.
  - Commercial = 8 + 40% of residents − open commercial jobs.
  - Industrial = 15 + 60% of residents − open industrial jobs.
  The bases let an empty map start a village. The shares add up to 100%, so a balanced town
  keeps every demand positive and keeps growing; overbuilding one use pushes it negative.
  These are tuning constants, not a configuration system.
- Open jobs are the ADR 0021 job positions of active buildings that are not closing. Their
  totals are kept per use and updated where a building is built, starts or stops closing, or
  is removed. They follow from hashed state, so they are not hashed; tests check them
  against a full scan.
- A lot whose construction comes due builds only if its use has positive demand. Otherwise
  it keeps its zone and checks again 10–30 sim seconds later, drawn from the lot, tick and
  seed. The deadline stays in the hashed per-lot schedule. A building can overshoot demand by
  its own size; the next lot then waits.
- A new building registers its homes and jobs on the tick it is built, so the next lot due
  that tick sees them.
- Demand never removes or closes existing buildings. Replacement growth after closure
  (ADR 0023) goes through the same gate.
- Stats report demand; the HUD draws it as R/C/I bars. `Config::ignore_demand` builds every
  due lot for tests that need full single-use blocks; the game leaves it off.

## Consequences

A town zoned for one use stops growing until the others are zoned, which is the point. Each
waiting lot costs one demand check per retry and nothing between retries. Large single
buildings (an apartment block of 150 people) can push demand well below zero for a while.
There is no land value, traffic, happiness or density preference in demand yet (M5), and
negative demand does not empty buildings (M6 redevelopment).
