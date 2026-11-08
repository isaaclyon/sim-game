# 0057. Data overlays show road congestion and service gaps

- Status: Accepted; implemented
- Date: 2026-11-08
- Extends: [0037](0037-pollution-and-land-value-steer-growth.md), [0041](0041-power-grids-follow-roads.md),
  [0052](0052-water-follows-roads-and-supplies-dense-buildings.md),
  [0053](0053-fire-stations-send-trucks-through-traffic.md)

## Context

M5 asks for data overlays: traffic, coverage, land value and happiness. Land value is already a
ground overlay (ADR 0037). Traffic only recoloured each car by its speed, which shows where
cars are slow at this instant but not which roads back up. Coverage existed as three separate
marker switches (power, water, fire), none of which answers "which buildings are missing
something?" at a glance. Happiness is being built separately (ADR 0056) and brings its own
overlay. The frontend must not compute game state, so both new views need sim-owned answers.

## Decision

- **Two more ground overlays.** The Views list's one-at-a-time overlays gain Traffic and
  Services after land value; P cycles through them too. Car colours and the service markers
  stay as separate switches.
- **Traffic.** The sim writes `[ax, ay, bx, by, class, road, jam]` per rendered segment of
  each road with a car on it. `jam` is how backed up the road is: per direction, each car's
  lost speed (1 stopped, 0 at the limit) times the car spacing, summed over the lane's length,
  capped at 1; the worse direction wins. Empty roads are left out. The worker sends it at most
  once a real second while running, and at once on start, edits and pause. The page eases each
  road's colour halfway toward each new reading, keyed by road ID, so lights cycling do not
  flash roads between green and red; this is display smoothing only.
- **Services.** The sim writes `[gaps, n, x0, y0, …]` per standing building: bits for no power,
  no water (dense buildings only) and no fire cover, then its lot outline. Fire cover only
  counts while fires are on and the city is past the fire threshold, matching when fires can
  start. The worker sends it when power, water, fire, lots, buildings or that fire rule change,
  coalesced like buildings. The page fills each lot green (every service), amber (one
  missing) or red (more).
- Both are read-only queries: nothing is stored, saved or hashed, and replays do not change.

## Consequences

- Players can find bottleneck roads and underserved blocks without switching between marker
  views. The traffic reading costs one pass over lanes and queued cars per second.
- Later services (police, health, schools) add a gap bit; the colour rule counts bits, so the
  page needs only the new bit's name.
- The happiness overlay (ADR 0056) joins the same list when that work lands.
