# 0027. Display-only scenery, building kit and agent looks

- Status: Accepted
- Date: 2026-11-04

## Context

Issue #27 replaces flat extruded boxes and a dark background with a low-poly look: ground
patches, a river and lake, woods, fields, street trees, and buildings with roofs, windows,
shopfronts and loading doors. ADR 0003 keeps game state in the sim. Water and trees do not
affect play yet, and later eras (village, town) need to restyle buildings without new sim data.

## Decision

- Ground, water, fields and trees are decoration placed by the frontend (`web/src/scenery/`)
  from the map extent and fixed seeds. They never feed sim state. Wild trees and fields hide
  where roads, zones or lots now are; street trees line streets and main roads that front
  residential or commercial zones. Roads may still cross drawn water.
- Buildings stay sim-owned: footprint, height, use and roof choice. The building snapshot adds
  `front`, the footprint edge facing the lot's road, derived when the snapshot is written and
  not stored or hashed. The format is `[zone, height, roof, front, n, x0, y0, …]`.
- The frontend kit (`web/src/building-kit/`) turns each record into roofs, windows, doors,
  awnings and rooftop details. Visual variety comes from a hash of the footprint, never a sim
  RNG. Colours and proportions live in a `Style`; one present-day style ships, and an era
  switch later picks another `Style` without sim changes.
- Building meshes are built per 256 m tile and rebuilt only when that tile's records change.
- Vehicle models (car, hatchback, SUV, van, pickup, box van), paint and walker clothes are
  chosen in the frontend from a hash of the stable agent id (issue #29), so a citizen keeps the
  same car and look. Every model is at most 6 m long to fit the sim's 7 m queue spacing. Cars
  show paint by default; a traffic view (T) tints them by speed instead. Walkers swing their
  legs by distance moved, so they stand still while waiting or paused. Freight trucks that are
  longer or slower would be sim-owned and need their own ADR.
- Cars and walkers outside the camera's view are not drawn. Far ones draw simpler, chosen by
  on-screen size (`agent-lod.ts`): a two-box car, a one-block walker, and no walker at all
  below a few pixels. Picking and follow use every agent's position regardless.
- Lane paint follows road class from the roads snapshot: main roads get a double solid yellow
  centre line and white edge lines, streets a dashed yellow centre line, alleys none. Paint
  stops at junctions where three or more segments meet. Traffic signals draw as mast arms
  from the signals snapshot, with the lamps its state calls for lit. Both are display only.
- Buildings carry trim from the same `Style`: a plinth, window frames with sills, framed doors
  on a step, and a cornice under flat roofs. Glass office blocks and factories skip window
  frames. Trim roughly doubles building triangles, so new detail should stay per wall or per
  opening, not per brick.

## Consequences

- If water or woods start to block roads or need clearing, the sim must own them, with a new
  snapshot and a superseding ADR. Until then a road can run over a drawn river.
- The scenery's look depends on the road bounds at start; issue #88 (empty land) will pass a
  fixed map extent instead.
- Any change to the building record layout must update the Rust writer, `protocol.ts`, the kit
  reader and `BuildingLayer` together.
