# 0052. Water follows roads and supplies dense buildings

- Status: Accepted; not yet implemented
- Date: 2026-11-06
- Extends: [0041](0041-power-grids-follow-roads.md), [0042](0042-roadside-pieces-snap-to-roads.md),
  [0036](0036-money-build-costs-taxes-upkeep.md), [0037](0037-pollution-and-land-value-steer-growth.md),
  [0033](0033-single-city-state-saves.md)

## Context

M5 lists water as the next service. Power (ADR 0041) already gives a network that follows
roads, player-placed sources and a growth gate, so water must add something new rather than
repeat it. The arc of the game is village to town: villages should not need waterworks, and
towns that build up should. The sim has a ground pollution grid (ADR 0037) but no real lakes
or rivers; drawn water is decoration (ADR 0027). Power plants are still free to build and run,
so utilities never touch the budget (ADR 0036).

## Decision

- **Network.** Every road carries water pipes. A water network is a set of roads joined by
  road, with the pumps and towers attached to them, derived from roads and pieces and not
  stored. There is no pipe tool and no line that joins separate road networks; each needs
  its own pump.
- **Pump.** Placed by the player beside a road, snapping and facing it like coal (ADR 0042).
  Its footprint clears zone cells and buildings like a plant. Output is a fixed capacity
  scaled by the ground pollution under its footprint: full on clean ground, falling linearly
  to 25% at full pollution. It needs no electricity.
- **Water tower.** Placed beside a road the same way. It produces nothing. Each update it
  fills from its network's spare pump output, up to a fixed capacity, and drains to cover
  a shortfall. Its fill level is authoritative state, saved and hashed.
- **Demand.** Only high-density buildings use water: dense residential, commercial and
  industrial, weighted by their homes and jobs. Low-density buildings are on wells and
  ignore water entirely.
- **Allocation.** Each network serves its dense buildings oldest first, by building ID,
  from pump output then tower storage; the rest are dry. There is no starter water.
- **Growth gate.** A dense lot builds only if it would get water, in addition to demand,
  land value and power. A scheduled lot remembers when water was the reason it waits, like
  power's waiting warning.
- **Drying out.** A dry dense building keeps everyone for one clock day. After that grace
  period, one household or one job leaves it every few clock hours (a tuning value) while it
  stays dry. Households that leave move out of the city. Workers whose job goes keep their
  home and look for work like any unemployed resident. A building that empties completely
  stays standing; when water returns its homes and jobs fill again normally. Water never
  closes or removes a building. The dry-since tick per building is saved and hashed.
- **Money.** Pumps and towers have build costs and upkeep per clock hour, and placing one is
  refused when the city cannot pay, like roads. In the same change power plants get build
  costs and upkeep (superseding "plants are free" in ADR 0041); power lines stay free. The
  money panel lists utility upkeep beside road upkeep. Exact prices are tuning values.
- **Bulldoze and undo.** Bulldoze removes pumps and towers like plants. They are placeables
  under the edit history of [0051](0051-road-edit-history-revised.md) once that lands.
- **Saves.** Pumps, towers, tower levels and dry-since ticks enter the save and state hash.
  The save version is bumped and older saves and links are refused (ADR 0033). If ADR 0051's
  history lands first, whichever branch merges second takes the next version number.
- **Display.** The top bar gets a water figure (supply, use, storage) that turns red during a
  shortage, with a details panel; a water view colours networks; dry dense buildings get a
  marker. Commands and snapshot formats are fixed when implemented and listed in `CLAUDE.md`.

## Consequences

- A village grows on wells alone. Painting dense zones starts needing pumps, so water marks
  the step from village to town.
- Ground pollution gains a second effect: a pump beside industry or coal gives much less
  water, so placement matters. Old factory sites stay poor pump sites for days.
- A water shortage is the first way sandbox can lose population, slowly and recoverably.
- Utilities now cost money to build and run, which gives taxes something to pay for.
- Every city saved before this change stops loading.
- Later slices: sewage (outlets that add ground pollution), real water bodies, pumps that
  need power, and happiness from water service.
