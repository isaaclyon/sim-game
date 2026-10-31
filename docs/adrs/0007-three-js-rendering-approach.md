# 0007. Three.js rendering approach

- Status: Accepted
- Date: 2026-10-30

## Context

The frontend must draw tens of thousands of moving vehicles at 60+ fps and provide a
city-builder camera. We'd rather adopt maintained libraries than hand-roll these parts.

## Decision

- **Renderer:** `WebGLRenderer` for now. `WebGPURenderer` is still marked experimental in the
  three.js manual. New custom shaders should be written in TSL so a later switch is cheap.
- **Camera:** `camera-controls` (MIT). Left-drag pans, right-drag orbits, wheel zooms to cursor,
  with bounds and a tilt limit. We evaluated MapControls (core) and found it cruder.
- **Vehicles:** one `InstancedMesh` of boxes, with matrices and colors written from the snapshot
  each frame. Position and heading are interpolated between snapshots.
- **Roads:** `LineSegments` colored by speed limit, rebuilt when the road version changes.
- **Coordinates:** sim `(x, y)` maps to world `(x, z)` with `y` up.
- **Picking:** ray against the ground plane for now. Add `three-mesh-bvh` once we pick against
  real meshes.

## Consequences

- One draw call for all vehicles. 50k agents ran at ~165 fps in testing.
- Writing 50k instance matrices on the CPU is the main per-frame cost. If it becomes the
  bottleneck, move interpolation and orientation into a vertex shader fed by the raw snapshot.
- Roads as lines are a placeholder. Road meshes, intersections and buildings will need
  `BatchedMesh`, `bezierjs` for curves, and `earcut` for polygon triangulation.
