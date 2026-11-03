// Boundaries enclose a property before loose props are placed. Only real obstructions
// (buildings, roads and the entrance) cut openings; random choice never removes a side.
import * as THREE from 'three';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { convex, inside, offset, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { fits } from './placement.ts';

const hedge = new THREE.Color(0x56794d);
const steel = new THREE.Color(0x737d80);
const white = new THREE.Color(0xeee9dc);
const block = new THREE.Color(0xa39d92);
export type BoundaryStyle = 'hedge' | 'metal' | 'picket' | 'wall' | 'none';
const mix = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];

/** Joined strips share their corner points, including at angled and concave corners. */
export function boundary(
  mb: MeshBuilder,
  lot: Pt[],
  obstacles: Pt[][],
  roads: Clearance,
  style: BoundaryStyle,
): Pt[][] {
  if (style === 'none') return [];
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
      if (style === 'hedge') mb.prism(shape, 0, 0.9, hedge, true);
      else if (style === 'wall') mb.prism(shape, 0, 2.1, block, true);
      else fence(mb, shape, style === 'picket');
    }
  }
  return reserved;
}

/** Open panels share solid posts and rails. Pickets have pointed, two-sided board faces. */
function fence(mb: MeshBuilder, strip: Pt[], picket: boolean) {
  const [a, b] = [mix(strip[0], strip[3], 0.5), mix(strip[1], strip[2], 0.5)];
  const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
  if (length < 0.1) return;
  const n: Pt = [-(b[1] - a[1]) / length, (b[0] - a[0]) / length];
  const side = (p: Pt, d: number): Pt => [p[0] + n[0] * d, p[1] + n[1] * d];
  const rail = [side(a, -0.04), side(b, -0.04), side(b, 0.04), side(a, 0.04)];
  const color = picket ? white : steel;
  for (const h of picket ? [0.3, 0.7] : [0.45, 1.65]) mb.prism(rail, h, h + 0.08, color, true);
  const bars = Math.max(1, Math.ceil(length / (picket ? 0.5 : 0.7)));
  const u: Pt = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
  for (let k = 0; k <= bars; k++) {
    const c = mix(a, b, k / bars);
    const post = k === 0 || k === bars || k % (picket ? 5 : 4) === 0;
    if (!picket || post) {
      // Upright cylinders keep their full footprint inside the reserved boundary strip.
      mb.cylinder(c, 0, post ? 0.065 : 0.025, picket ? 1.08 : post ? 1.85 : 1.73, 4, color);
    } else {
      // Flat boards avoid six box faces per picket. Both sides are visible; rails give depth.
      const w = Math.min(0.09, length / bars / 3);
      const left: Pt = [c[0] - u[0] * w, c[1] - u[1] * w];
      const right: Pt = [c[0] + u[0] * w, c[1] + u[1] * w];
      for (const sign of [-1, 1]) {
        const normal: [number, number, number] = [n[0] * sign, 0, n[1] * sign];
        mb.quad(
          [left[0], 0.06, left[1]],
          [right[0], 0.06, right[1]],
          [right[0], 0.84, right[1]],
          [left[0], 0.84, left[1]],
          color,
          normal,
        );
        mb.tri(
          [left[0], 0.84, left[1]],
          [right[0], 0.84, right[1]],
          [c[0], 1, c[1]],
          color,
          normal,
        );
      }
    }
  }
}
