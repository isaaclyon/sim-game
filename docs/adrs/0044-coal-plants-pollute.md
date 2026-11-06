# 0044. Coal plants pollute

- Status: Accepted
- Date: 2026-11-05
- Supersedes: the "no new pollution sources" part of [0041](0041-power-grids-follow-roads.md)
- Extends: [0037](0037-pollution-and-land-value-steer-growth.md)

## Context

ADR 0041 kept power plants out of the pollution system, so coal was the best plant with no
downside: large, steady output, free, and clean. Solar and wind had only drawbacks next to
it. Pollution (ADR 0037) already lowers land value and steers homes away from dirty land, so
it is the natural cost for coal.

## Decision

- Every standing coal plant is a pollution source, fed the same way as a factory: air and
  noise spread over the cells under its footprint's bounds, ground pollution builds up under
  it and at half strength in the ring of cells around it.
- A coal plant emits a fixed amount, stronger in air than the largest factory and quieter.
  It does not depend on output, staffing or demand, as coal output does not (ADR 0041).
- Solar and wind emit nothing.
- Removing a plant stops its air and noise at the next pollution update; its ground
  pollution fades like a factory's.

## Consequences

Coal now trades clean land for steady power: homes near it grow late, and a demolished coal
site stays tainted for a few sim days. The pollution overlays and haze show it with no
frontend or snapshot change. The save format is unchanged, because plants and pollution are
already saved; the state hash of any city with a coal plant changes, so replay fixtures with
coal were re-blessed once. The chimney smoke drawn over a coal plant is display only and not
part of this decision.
