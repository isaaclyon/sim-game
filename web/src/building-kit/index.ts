// Building footprints, heights and fronts come from the sim. The kit only shapes and colours.
import type * as THREE from 'three';
import { Era } from '../protocol.ts';
import { building } from './kit.ts';
import { MeshBuilder } from './mesh.ts';
import { ccw, type Pt } from './polygon.ts';
import { PRESENT_DAY, type Style, VILLAGE } from './style.ts';

/**
 * Reads `[zone, height, roof, front, era, n, x0, y0, …]` per building. Sim (x, y) becomes local
 * (x, z). Village buildings take the village look whatever `style` the town uses (ADR 0056).
 */
export function buildBuildingGeometry(
  data: Float32Array,
  style: Style = PRESENT_DAY,
  options?: { compact?: boolean },
): THREE.BufferGeometry {
  const mb = new MeshBuilder(options);
  const groups: { start: number; count: number }[] = [];
  let o = 0;
  while (o < data.length) {
    const [zone, height, roof, front, era, count] = data.subarray(o, o + 6);
    o += 6;
    const ring: Pt[] = [];
    for (let i = 0; i < count; i++, o += 2) ring.push([data[o], data[o + 1]]);
    // Keep one triangle range per snapshot record for picking, including empty ranges.
    const start = mb.indexCount;
    // The sim sends counter-clockwise rings; reversing would move the front edge.
    if (ring.length >= 3 && ring === ccw(ring))
      building(mb, { zone, height, roof, front, era, ring }, era === Era.Village ? VILLAGE : style);
    groups.push({ start, count: mb.indexCount - start });
  }
  const geometry = mb.geometry();
  for (const { start, count } of groups) geometry.addGroup(start, count, 0);
  return geometry;
}
