# 0017. Road classes and main-road routing

- Status: Accepted
- Date: 2026-11-02

## Context

Until now a road's speed limit was its only kind. The generated grid made every fifth row and
column a faster avenue, junctions treated "faster than a street" as "the bigger road", and
players could only build streets. Route search (ADR 0010) runs weighted A* over every road for
every trip, so long trips expand most of the streets between their ends, and drivers pick
whichever streets add up cheapest instead of heading for a main road the way people do.

The roadmap (M2) asks for road classes: trips use local streets to reach a main road, ride it,
then leave it near the destination, and the search uses the main-road layer for the middle of
long trips. Multi-lane roads and highways are a separate, later issue (#52).

## Decision

- **Road class is per road and explicit.** `RoadClass` is `Street` (0), `Main` (1) or
  `Alley` (2, laid by the sim to reach deep lots, ADR 0016). It sets
  the speed limit (13.9 and 20 m/s), right of way at priority junctions, and which 4+-way
  junctions get signals. All still have one lane each way; lane counts can later hang off
  the class or the road without changing this.
- **Commands carry the class.** `BuildRoad` and `BuildCurve` take a `class`. Where a build
  follows an existing road, that road keeps its class; a split road's two halves keep theirs.
  The WASM boundary passes it as a `u8` and refuses unknown values.
- **Road snapshot field 5 is the class**, replacing `speed_limit` (which the renderer never
  used): `[ax, ay, bx, by, class]` per rendered segment. The renderer draws main roads wider
  (12 m instead of 8 m); cars keep the same lane offsets.
- **Main-road layer search.** `Pathfinder::find` first runs A* that, at any node farther than
  `MAIN_ROAD_REACH` (500 m, straight line) from both the trip's start and its end, follows only
  main roads. Near either end every road is usable, so a trip takes local streets to a main
  road, rides it, and leaves it near the destination. If that search finds nothing (no main
  road within reach of an end, or none linking them), it reruns over every road, so any
  reachable destination is still found. Short trips rarely reach the restricted zone and
  route as before. Networks with no main roads skip the first search.
- Cost, heuristic, queue delays, detours and determinism are as in ADR 0010. Detours remain
  local searches over every road.

## Consequences

- Measured with `traffic_bench` (Apple Silicon, deterministic counts; timings were noisy on a
  shared machine):
  - Grid, 50k agents, 1 sim-hour: nodes expanded per route search fell from about 690 to 350.
    Trips completed fell about 11% (90k to 81k); stuck despawns stayed at 0-3.
  - LA scale (`--scale la`): nodes per search fell from about 3,500 to 1,800. Trips completed
    fell about 30% (44k to 31k); stuck despawns rose from 4 to 11-16 per 20 sim-minutes.
  - At low density on the same 200x200 map (30k agents) trips fell under 2%, so the LA loss is
    congestion, not longer routes: long trips now share the single-lane avenues, 1 in 5 rows
    and columns, instead of spreading over every street. Raising the reach to 1,200 m only
    recovered a third of it.
  - The hand-drawn organic layout has no main roads and is unchanged.
  - Interleaved ms/tick runs differed by less than the run-to-run noise noted in ADR 0012.
- Main roads need more capacity than one lane each way to carry what this routing sends them.
  Multi-lane roads (#52) and per-driver route noise (#20) are the expected fixes; retuning
  `MAIN_ROAD_REACH` is the quick lever.
- A failed route search now costs up to two searches. Failures are rare (about 1% at LA scale,
  from streets the generator drops).
- Routes are less optimal than before by design: a long trip may take a main road that is a
  little slower than the straight street route, as a human driver would.
- Road snapshot readers must treat field 5 as a class, not a speed. This supersedes the
  `speed_limit` field of ADR 0003 and ADR 0015; their record cardinality stands.
