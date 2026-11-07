# 0050. Clicking a road changes its class

- Status: Accepted; implemented
- Date: 2026-11-06
- Extends: [0048](0048-drawing-over-roads-changes-their-class.md)

## Context

M4.5 slice 4 asks for a player to click a road to switch it between street and main road, and
to turn an alley into a street. ADR 0048 lets a drawn road change the class of the roads it
follows, and left single clicks out. A click could be sent as a `BuildRoad` between the road's
ends, but the frontend would have to find those ends from preview samples, and that only works
for straight roads: a straight line between a curved road's ends follows the curve only when
nothing else lies between them. The frontend must not work out game state ([ADR
0003](0003-sim-and-rendering-are-separate.md)), so the sim needs to pick the road.

## Decision

- **A new command**, `SetRoadClass { x, y, class }`: the road nearest the point, within the
  bulldozer's reach (`snap::bulldoze_target`), takes `class`. Plants, lines and buildings do not
  block the click; only roads are targets.
- **The same change as drawing.** The sim plans the click as drawing a road of `class` over
  that whole road from end to end, and builds that plan through ADR 0048's code path. So the
  rules are 0048's: the road keeps its ID, lanes, ends and shape; its lanes take the new speed
  limit; cars keep their routes; junction controls are rebuilt; roads that now share a class
  and meet end to end in a line may join (ADR 0040), which removes the cars on those two roads
  as any join does. Upgrading pays the difference in build rate, downgrading is free with no
  refund, and a change the city cannot afford is refused.
- A click on a road that already has `class`, or away from every road, changes nothing and is
  refused.
- **Command logs** write it as `class x y street|main|alley`. The log version stays 1: older
  logs have no such line and read the same. Saves do not change.
- **Preview.** `World::write_class_preview` writes `[x, y]` centreline samples of the road a
  click would change, like the bulldoze preview, and nothing when the click would change
  nothing; `World::class_cost` prices it. The worker answers `previewRoadClass` with
  `roadClassPreview { segment, cost, affordable, revision }`.
- **Interface.** Upgrade is an option of the road items, beside Straight, Curve and Continuous.
  The selected item (Street or Main road) is the class a clicked road takes; M switches it as
  before. No new key.

## Consequences

- A build made before this change never contains `class` lines, so every existing replay keeps
  its expectations.
- An older build cannot read a log with `class` lines; it reports an unknown command.
- Undo (ADR 0039, proposed) must record these class changes, as it must for 0048's.
- Clicking cannot reach alleys' cheaper class: there is no alley item, so alleys only become
  streets, as the roadmap asks.
