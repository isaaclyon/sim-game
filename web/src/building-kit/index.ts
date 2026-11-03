// Building footprints, heights and fronts come from the sim. The kit only shapes and colours.
import type * as THREE from 'three';
import { building } from './kit.ts';
import { MeshBuilder } from './mesh.ts';
import { ccw, type Pt } from './polygon.ts';
import { PRESENT_DAY, type Style } from './style.ts';

/** Reads `[zone, height, roof, front, n, x0, y0, …]` per building. Sim (x, y) becomes local (x, z). */
export function buildBuildingGeometry(
  data: Float32Array,
  style: Style = PRESENT_DAY,
): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  const groups: { start: number; count: number }[] = [];
  let o = 0;
  while (o < data.length) {
    const [zone, height, roof, front, count] = data.subarray(o, o + 5);
    o += 5;
    const ring: Pt[] = [];
    for (let i = 0; i < count; i++, o += 2) ring.push([data[o], data[o + 1]]);
    // Keep one triangle range per snapshot record for picking, including empty ranges.
    const start = mb.vertexCount;
    // The sim sends counter-clockwise rings; reversing would move the front edge.
    if (ring.length >= 3 && ring === ccw(ring))
      building(mb, { zone, height, roof, front, ring }, style);
    groups.push({ start, count: mb.vertexCount - start });
  }
  const geometry = mb.geometry();
  for (const { start, count } of groups) geometry.addGroup(start, count, 0);
  return geometry;
}
