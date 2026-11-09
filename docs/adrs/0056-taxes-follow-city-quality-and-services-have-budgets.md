# 0056. Taxes follow city quality, and each service building has a budget

- Status: Accepted; not yet implemented
- Date: 2026-11-09
- Extends: [0036](0036-money-build-costs-taxes-upkeep.md), [0037](0037-pollution-and-land-value-steer-growth.md),
  [0053](0053-fire-stations-send-trucks-through-traffic.md), [0055](0055-schools-and-education-levels.md)

## Context

Money (ADR 0036) slows growth but gives the player few choices: every resident pays $2 an hour
and every filled job $1, whatever the city is like, and a service building costs the same
whether it is busy or idle. Land value (ADR 0037) and education (ADR 0055) now describe how
good a place is, but neither earns the city anything. The owner wants money to reward good
planning and to offer trade-offs, not to cause problems: debt keeps its ADR 0036 meaning
(only spending stops) for now.

## Decision

- **Order.** Schools (ADR 0055) first, then three slices: taxes that follow city quality,
  then service budgets, then tax rates after happiness (#39).
- **Taxes follow city quality.** Each resident's tax scales with their home's land value;
  each filled job's tax scales with its worker's education level. The scale is fixed
  constants chosen so a typical home and worker pay today's $2 and $1, with better places
  paying more and worse ones less. The reference is calibrated once from a test city, not
  recomputed from the live city, so a well-planned city really does earn more.
- **Service budgets are per building, in units.** A fire station's budget is its number of
  trucks; a school's is its number of seat blocks. The player sets units directly on the
  building, within limits per kind. Upkeep and staff jobs are per unit, so a cut saves money
  and frees workers, who search for jobs as usual (ADR 0021). A new building starts at a
  standard number of units, and the build menu's price is for that standard.
- **Cuts take effect gradually.** Children already enrolled keep their seats; a school over
  its seats takes no new pupils until it is under them. A truck out on a call finishes it
  before it is retired. Coverage areas do not change with budgets.
- **Tax rates wait for happiness.** Later, the player sets three tax rates: residential,
  commercial and industry. Higher rates raise money and cost happiness. There is no interim
  cost (such as lower demand), so rates are tuned once, against happiness.
- **Saves.** Units per building enter the save and state hash; changing them is a command.
  The save version is bumped when that slice lands and older saves are refused (ADR 0033).

## Consequences

- ADR 0055 schools must keep seats and teacher jobs per seat block, so budgets can be added
  without redesigning them.
- Land value matters for money as well as growth, so pollution and fire coverage have a
  cost the player can read in the money panel. If land value varies little across a city,
  the tax spread will be hard to see; a balance run after schools will show this.
- Per-building budgets mean more clicks than one budget per service; the inspector card
  (#104) is where they are set.
- Rejected: a budget that shrinks coverage areas (homes at the edge flip in and out of
  service), one budget per service (no way to fund a busy station more), fixed upkeep
  whatever the budget (no reason to lower it), and taxes that cost demand before happiness
  exists (tuned twice).
- Debt penalties, loans and district policies (#61) stay out of scope.
