# 0048. Drawing a road over existing roads changes their class

- Status: Accepted; implemented
- Date: 2026-11-05
- Supersedes: the class-keeping rule of [0017](0017-road-classes-and-main-road-routing.md) and
  the free followed legs of [0036](0036-money-build-costs-taxes-upkeep.md)

## Context

M4.5 slice 4 asks for road upgrades: switch roads between street and main road, let alleys
become streets, and keep cars driving. Until now a build that followed an existing road kept
that road's class (ADR 0017), followed legs cost nothing (ADR 0036), and a build that only
followed roads added nothing and was refused. A straight line drawn along a road without
crossing anything was refused as an overlap.

## Decision

- **Drawing is upgrading.** `BuildRoad` and `BuildCurve` give every road the plan follows the
  drawn class. Only whole roads between stops change; a line that starts or ends part-way
  along a road it runs on is refused, as before. No new command: command logs replay
  upgrades as ordinary builds.
- A plan is valid if it adds a road **or** changes a followed road's class. Drawing over roads
  of the same class still adds nothing and is refused.
- A straight line that runs along a road (the existing overlap test) gets that road's ends
  as stops, so it follows the road and may run on past it. A curve over a road is still
  refused; a straight line between a curved road's ends follows the curve.
- **Cost:** new legs pay the full build rate. A followed road moved to a dearer class pays
  the difference in build rate for its length ($15/m street to main, $5/m alley to street).
  Moving to a cheaper class is free and refunds nothing. Upkeep follows the new class.
  A change the city cannot afford is refused like any build.
- **In place:** the road keeps its ID, lanes, ends and shape; its lanes take the new speed
  limit. Cars keep their routes and are not removed. Junction controls (signals, right of
  way) are rebuilt, and roads that now share a class may join (ADR 0040).
- Alleys and connector streets the sim cuts into deep blocks keep the class of any road they
  follow.
- Previews take the class (`write_road_draft`, the WASM `road_draft`), because whether a
  draft is valid and what it costs now depends on it. The worker passes the class
  `previewRoad` already carries.

## Consequences

- Saves keep their format. Replays are unchanged unless they draw over roads of another
  class, or along a road without crossing it; those builds used to be refused.
- Lots do not depend on class, so upgrading keeps every lot and building. Buildings that
  grew beside a street keep their street setback, so a few may sit closer to the wider main
  road than new buildings would.
- Undo (ADR 0039, proposed) must record class changes as well as added and split roads.
- Clicking a single road to switch it, without drawing, is not part of this decision.
