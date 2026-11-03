// Boundaries enclose a property before loose props are placed. Only real obstructions
// (buildings, roads and the entrance) cut openings; random choice never removes a side.
import * as THREE from 'three';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { convex, inside, offset, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { fits } from './placement.ts';

const hedge = new THREE.Color(0x56794d);
const steel = new THREE.Color(0x737d80);
const mix = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];

/** Joined strips share their corner points, including at angled and concave corners. */
export function boundary(
  mb: MeshBuilder,
  lot: Pt[],
  obstacles: Pt[][],
  roads: Clearance,
  industrial: boolean,
): Pt[][] {
  const outer = offset(lot, -0.2);
  const inner = offset(lot, -0.65);
  const reserved: Pt[][] = [];
  for (let i = 0; i < lot.length; i++) {
    const j = (i + 1) % lot.length;
    const length = Math.hypot(lot[j][0] - lot[i][0], lot[j][1] - lot[i][1]);
    const strip = (a: number, b: number): Pt[] => [
      mix(outer[i], outer[j], a),
      mix(outer[i], outer[j], b),
      mix(inner[i], inner[j], b),
      mix(inner[i], inner[j], a),
    ];
    const runs: [number, number][] = [];
    const cut = (a: number, b: number) => {
      const shape = strip(a, b);
      if (fits(shape, lot, obstacles, roads)) {
        const last = runs.at(-1);
        if (last && Math.abs(last[1] - a) < 1e-8) last[1] = b;
        else runs.push([a, b]);
        return;
      }
      if ((b - a) * length < 0.2) return;
      if (obstacles.some((ring) => convex(ring) && shape.every((p) => inside(p, ring)))) return;
      const middle = (a + b) / 2;
      cut(a, middle);
      cut(middle, b);
    };
    cut(0, 1);
    for (const [a, b] of runs) {
      const shape = strip(a, b);
      reserved.push(shape);
      if (industrial) fence(mb, shape);
      else mb.prism(shape, 0, 0.9, hedge, true);
    }
  }
  return reserved;
}

/** Open metal panels: posts, two rails and slim vertical bars, not a solid green wall. */
function fence(mb: MeshBuilder, strip: Pt[]) {
  const [a, b] = [mix(strip[0], strip[3], 0.5), mix(strip[1], strip[2], 0.5)];
  const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
  if (length < 0.1) return;
  const n: Pt = [-(b[1] - a[1]) / length, (b[0] - a[0]) / length];
  const side = (p: Pt, d: number): Pt => [p[0] + n[0] * d, p[1] + n[1] * d];
  const rail = [side(a, -0.04), side(b, -0.04), side(b, 0.04), side(a, 0.04)];
  for (const h of [0.45, 1.65]) mb.prism(rail, h, h + 0.08, steel, true);
  const bars = Math.max(1, Math.ceil(length / 0.7));
  for (let k = 0; k <= bars; k++) {
    const c = mix(a, b, k / bars);
    const post = k === 0 || k === bars || k % 4 === 0;
    // Upright cylinders keep their full footprint inside the reserved boundary strip.
    mb.cylinder(c, 0, post ? 0.065 : 0.025, post ? 1.85 : 1.73, 4, steel);
  }
}
