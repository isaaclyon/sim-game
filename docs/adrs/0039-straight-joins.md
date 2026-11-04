# 0039. Straight roads that meet end to end in a line become one road

- Status: Accepted
- Date: 2026-11-04

## Context

A street drawn in several clicks, or a through road left behind when its side street is
bulldozed, used to keep a node where the pieces met. Nothing marks such a node on screen, yet
road ends within 15 m snap to it (`snap::SNAP_RADIUS`). That beats angle and guide snapping
(ADR 0037): a road aimed square at such a street bends to meet the hidden node instead. Deleting
roads also left nodes no road uses, and those pulled new roads too.

## Decision

- After every player road build and bulldoze, any node where exactly two active roads meet is
  joined if both are straight, of one class, and in line to within a sine of 0.001 (about
  0.06°). The lower-numbered road is extended in place over the other, which is tombstoned
  (ADR 0013). IDs stay append-only and the shared node stays, unused.
- The pass covers the whole network, so cities saved before this lose their hidden straight
  joins at the first road edit. Generated grids and alley cuts have no such joins and are not
  changed on their own.
- Lots fronting the removed road move to the kept one. Junction reservations move to its lanes.
  Cars routed over either road are removed, as for a split. A lot's front point may move by
  rounding, so lot matching compares it to within the outline tolerance (1 cm).
- Snapping ignores nodes no active road uses (`RoadNetwork::nearest_junction`).
- Curves and bends keep their node. A chain of curved pieces still has nodes along it; slice 2
  (continuous curves) must decide how those snap.

## Consequences

- Saves and command logs keep their format. Replays are unchanged unless they draw straight
  roads in pieces or bulldoze side streets; those now record fewer roads.
- Extending a street, or removing a side street, removes the cars on the whole joined street.
- Unused nodes accumulate, as they already did after bulldozing. Path destinations drawn from
  all nodes may land on one and fail, which parks the car for a retry as before.
