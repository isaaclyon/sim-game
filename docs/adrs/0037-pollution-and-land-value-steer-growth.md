# 0037. Pollution lowers land value, and land value steers growth

- Status: Accepted
- Date: 2026-11-04
- Extends: [0025](0025-rci-demand-gates-growth.md) and
  [0033](0033-single-city-state-saves.md) (save version 3)

## Context

ROADMAP M5 makes pollution an input to happiness and lists a land value overlay. Happiness,
money and services do not exist yet, but buildings, industry and traffic do. Growth today is
the ADR 0025 per-lot check: a due lot builds whenever its use has positive demand, wherever it
is. Nothing makes a home next to a factory or a busy main road less likely than a quiet one.
The sandbox must stay free of failure, and the sim must stay deterministic and cheap at
LA scale (ADR 0012).

Alternatives considered:

- Pollution as happiness only: needs happiness first, and makes residents leave rather than
  shaping where the city grows.
- Ranking waiting lots city-wide and giving demand to the cleanest: precise, but replaces the
  per-lot timer with a global queue.
- A hard pollution cutoff for homes: easy to read, but a cliff edge rather than a gradient.
- Each pollution kind feeding growth directly: every later factor (services, parks) would add
  another growth input.
- Recomputing everything from what stands now: no saved state, but no lingering ground
  pollution, and noise from live lane queues would make growth depend on the time of day.

## Decision

- **Three kinds.** Air pollution comes from active industrial buildings and from traffic flow
  on roads, and spreads evenly in every direction (no wind). Noise comes from industrial
  buildings and traffic flow, and is short range. Ground pollution builds up under and right
  beside active industrial buildings, up to a cap, and fades over several sim days after the
  building closes or is removed.
- **Traffic flow** is a smoothed per-road average of cars passing, not the cars on the road
  at the moment, so a lot reads the same at rush hour and at night within a small range.
- **Pollution grid.** Values live on a coarse grid aligned to the zone grid (ADR 0014),
  initially 4×4 zone cells per pollution cell. Lots read the cells they cover.
- **Land value** is one number per pollution cell. Pollution lowers it; clean land has full
  value. Later factors (services, parks) act through it, not as new growth inputs.
- **Growth gate.** ADR 0025's check stays per lot. A residential lot builds only while
  residential demand exceeds a threshold that rises as its land value falls; commercial lots
  use a milder threshold; industry ignores land value. Clean lots therefore grow first and
  dirty ones grow when demand is high.
- **Existing buildings stay.** Pollution and land value never close or empty a standing
  building. Moving away waits for happiness.
- **State.** Ground pollution per cell and traffic flow per road accumulate over time. Air,
  noise and land value are recomputed from them and the current sources on each update, every
  10 sim seconds. All of them are saved, because buildings can change between updates and
  growth reads the last update's land value; rebuilding it on load would let a loaded city
  diverge. Ground pollution, traffic flow and land value are part of `state_hash`; air and
  noise only feed land value and the display.
- **Older saves.** Saves carry the pollution grid, so the save version becomes 3. Version 1
  and 2 saves and links are refused, as ADR 0033 says. An earlier draft converted saves from
  before pollution; money (ADR 0036) took version 2 at the same time, so the conversion was
  dropped rather than written for both.
- **Display.** Overlays for air, ground, noise and land value, plus display-only haze and
  stained ground. The snapshot format is fixed in CLAUDE.md when it lands.
  Chimneys on factories that draw one also puff smoke. It is timed by the display clock
  alone, drawn for the 300 chimneys nearest the camera, and held still under reduced motion.

## Consequences

Industry and main roads push housing away without any way to fail, and old factory sites stay
tainted for a while after rezoning. Every growth check reads land value, so pollution must be
current enough at that point; recomputing it on a fixed sim-time cadence keeps the per-tick
cost small. Growth changes, so replay `expect` lines need blessing once, and the save version
bumps. Old cities and links no longer open. Happiness, health effects, wind,
clean-up tools and offices as clean jobs (M6) are later decisions that build on land value.
