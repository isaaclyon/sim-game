# 0031. Low-density commerce grows small shops, not full-lot blocks

- Status: Accepted
- Date: 2026-11-02

## Context

ADR 0018 gives all commercial buildings full-lot footprints and shared side walls. On a
standard block these lots reach about 56 m back from the street. Even a low building then
looks like a deep warehouse or shopping centre, rather than a neighbourhood shop. Lowering
its job count (ADR 0030) does not change that appearance.

## Decision

- Supersede only ADR 0018's low-density commercial footprint rule. Lots remain polygons cut
  along roads; high-density commerce still follows their shapes and shares side walls.
- Low-density commercial buildings use shallow road-facing rectangles fitted wholly inside
  their lots and clear of frontage and corner roads. Narrow or awkward sites shrink the box
  or stay open, rather than producing deep wedges. Most shops keep side gaps; occasional
  adjacent frontage slots can form a pair, not a continuous row. No lots are merged.
- The sim owns these footprints and heights. Current tuning caps width at 18 m and depth at
  16 m; four shops in five have one storey, the rest two. These proportions are tuning, not
  a new snapshot field or configuration option.
- Job capacity still follows ADR 0030: ground-floor footprint only, with a two-job minimum
  (staffing rate later retuned by [0032](0032-compact-shop-staffing.md)).
  The frontend does not invent extra tenants, workplaces or shop capacity.
- The existing building kit adds individual doors, glazing, awnings and shallow varied roofs.
  Small-shop outdoor space is planted, with a paved entrance and details only where they fit.
  These are display-only, remain tied to the standing building during closure, and use no
  gameplay RNG.

## Consequences

- Low and high commercial density differ in land coverage as well as height. Larger lots do
  not automatically become larger low-density shops, leaving space beside or behind them.
- Smaller trading floors reduce jobs per commercial block without changing capacity formulas
  or demand shares. The land-balance test checks that shops can still support several blocks
  of houses; inspection reports the actual capacity of each new footprint.
- Existing buildings keep their recorded shapes until removal. New cities and buildings use
  the new rules. Replaying the same commands under this version changes growth and hashes;
  there is no save migration yet.
- Snapshot formats, lot IDs, closure, placement clipping and the sim/frontend boundary stay
  unchanged.
