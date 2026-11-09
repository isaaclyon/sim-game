# 0058. Citizen happiness steers housing demand and move-outs

- Status: Accepted; implemented
- Date: 2026-11-08
- Extends: [0025](0025-rci-demand-gates-growth.md), [0037](0037-pollution-and-land-value-steer-growth.md),
  [0022](0022-citizen-inspection-query.md), [0033](0033-single-city-state-saves.md)

## Context

M5 asks for citizen happiness from services, traffic and pollution, feeding back into demand
and growth, with each person's mood and complaints in the citizen inspector. The sim already
knows who has power (ADR 0041) and water (ADR 0052), which buildings a fire station covers
(ADR 0053), air and noise per pollution cell (ADR 0037) and who has a job. It does not know
how long anyone's trips take, so traffic jams never reach the people stuck in them. Sandbox
must stay relaxed: problems slow growth or make people move away, with no game over.

## Decision

- **Mood is derived per citizen**, 0 to 100, from a neutral base, never stored per person.
  Home factors are shared by everyone in a home: no power, no water (dense homes only), fire
  cover (a bonus when covered; a complaint when uncovered while fires are on and the city is
  big enough to have them), air pollution and noise at the home. Adults add no job and a long
  commute. Children take only their home's factors. Each factor that takes points off is a
  complaint; the inspector lists them.
- **Commute time is new citizen state.** Each worker remembers when their current trip to
  work set out and how many clock minutes their last one took, including waiting for a route
  and retries. Losing the job clears it.
- **City happiness is recomputed on the pollution cadence** (ADR 0037, every 10 sim seconds),
  right after pollution, from the same derived power, water and cover caches. It stores the
  people-weighted average, how many residents have each complaint, and a mood byte per
  pollution cell for the overlay. All of it is saved, and the average, the counts and the
  move-out state are hashed, because demand reads the average until the next update.
- **Demand.** Residential demand (ADR 0025) gains a term proportional to the average's
  distance from neutral, scaled by a base plus a share of population. A happy city attracts
  people; an unhappy one turns them away.
- **Move-outs.** A household whose mean mood stays below a threshold for a clock day
  starts to leave the city: at each update after that, a small chance from a dedicated RNG
  stream. Recovering above the threshold resets the wait. Leaving uses the existing
  "household leaves the city" path, so homes become vacant and can refill.
- **Display.** Frame stats carry the average and complaint counts. The citizen inspector
  answers mood, complaint codes and commute minutes. The pollution message gains a parallel
  `mood` array, one byte per pollution cell (0 = nobody lives there, else 1 + mean mood ×
  254 / 100); the ground overlay list gains a happiness view. Complaint codes: 0 no power,
  1 no water, 2 no fire station nearby, 3 dirty air, 4 noise, 5 no job, 6 long commute.
- **Saves.** The save version goes up; older saves and links are refused (ADR 0033).

## Consequences

- Services, pollution and traffic now cost people, not just growth: an unpowered, dry or
  smoky district slowly empties, and a jammed commute turns workers unhappy.
- Replays change once, because demand and departures change; expectations are re-blessed.
- The update is a scan of every household on the pollution cadence: linear in residents,
  every 100 ticks at the default `dt`.
- Later services (police, health, schools) add factors and complaint codes; the inspector
  and stats carry codes, so the frontend must learn each new code.

## Implementation notes

- Tuning lives in `crates/sim/src/world/happiness/mood.rs` and `mod.rs`: base 60; no power
  −25; no water −25; fire cover +10, uncovered −10 at or above the fire threshold with fires
  on; air and noise each cost up to −20 and −15 above a quarter of full; no job −15; commutes
  above 40 clock minutes cost up to −20 at 100 minutes. Below 30 for a clock day, a household
  leaves with a 1-in-250 chance per update. Demand gains (average − 50) / 50 × (20 + 10% of
  population).
