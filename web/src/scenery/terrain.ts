// Flat ground meshes: patchy grass, water with sandy banks, and striped fields.
import * as THREE from 'three';
import { MeshBuilder } from '../building-kit/mesh.ts';
import type { Pt } from '../building-kit/polygon.ts';
import { GROUND, WATER } from '../palette.ts';
import { type Field, type Land, RIVER_HALF } from './land.ts';
import { hash2 } from './noise.ts';

/** Jittered triangles `cell` metres wide, each coloured by the patch under its centre. */
export function groundGeometry(land: Land, cell: number): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  const [x0, y0, x1, y1] = land.extent;
  const [i0, k0] = [Math.floor(x0 / cell), Math.floor(y0 / cell)];
  const [i1, k1] = [Math.ceil(x1 / cell), Math.ceil(y1 / cell)];
  // Inner vertices move up to a third of a cell; border vertices stay on the extent.
  const vertex = (i: number, k: number): Pt => {
    const edge = i === i0 || i === i1 || k === k0 || k === k1;
    const j = edge ? 0 : cell * 0.35;
    return [i * cell + (hash2(i * 3 + 1, k) - 0.5) * j, k * cell + (hash2(i, k * 3 + 2) - 0.5) * j];
  };
  const color = new THREE.Color();
  const tri = (a: Pt, b: Pt, c: Pt) => {
    const [cx, cy] = [(a[0] + b[0] + c[0]) / 3, (a[1] + b[1] + c[1]) / 3];
    color.set(GROUND[land.patch(cx, cy)]);
    // A faint per-triangle shade keeps large patches from looking flat.
    color.multiplyScalar(1 + (hash2(Math.floor(cx), Math.floor(cy)) - 0.5) * 0.035);
    mb.tri([a[0], 0, a[1]], [b[0], 0, b[1]], [c[0], 0, c[1]], color, [0, 1, 0]);
  };
  for (let i = i0; i < i1; i++)
    for (let k = k0; k < k1; k++) {
      const [a, b, c, d] = [vertex(i, k), vertex(i + 1, k), vertex(i + 1, k + 1), vertex(i, k + 1)];
      if (hash2(i, k) < 0.5) {
        tri(a, b, c);
        tri(a, c, d);
      } else {
        tri(a, b, d);
        tri(b, c, d);
      }
    }
  return mb.geometry();
}

/** Shore, bank, then three depths, each drawn over the last. */
const RINGS: [number, number][] = [
  [9, WATER.shore],
  [3, WATER.bank],
  [0, WATER.shallow],
  [-6, WATER.mid],
  [-16, WATER.deep],
];

export function waterGeometry(land: Land): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  const [, y0, , y1] = land.extent;
  const river: Pt[] = [];
  for (let y = y0; y <= y1 + 25; y += 25) river.push([land.riverX(y), y]);
  for (const [d, hex] of RINGS) {
    const color = new THREE.Color(hex);
    mb.poly(land.lakeRing(d), 0, color);
    // The river is narrower than the lake, so its deepest band is thinner.
    const half = RIVER_HALF + Math.max(d, -10);
    for (let i = 0; i + 1 < river.length; i++) ribbon(mb, river[i], river[i + 1], half, color);
  }
  return mb.geometry();
}

function ribbon(mb: MeshBuilder, a: Pt, b: Pt, half: number, color: THREE.Color) {
  const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const [nx, ny] = [(-(b[1] - a[1]) / l) * half, ((b[0] - a[0]) / l) * half];
  // Extend each piece by a little so bends leave no gaps.
  const [ex, ey] = [((b[0] - a[0]) / l) * half * 0.3, ((b[1] - a[1]) / l) * half * 0.3];
  const [p, q] = [
    [a[0] - ex, a[1] - ey],
    [b[0] + ex, b[1] + ey],
  ];
  mb.quad(
    [p[0] + nx, 0, p[1] + ny],
    [q[0] + nx, 0, q[1] + ny],
    [q[0] - nx, 0, q[1] - ny],
    [p[0] - nx, 0, p[1] - ny],
    color,
    [0, 1, 0],
  );
}

/** Crop rows 7 m apart, alternating slightly in shade. */
export function fieldsGeometry(fields: Field[]): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  const color = new THREE.Color();
  for (const { corners, crop } of fields) {
    const [a, b, , d] = corners;
    const depth = Math.hypot(d[0] - a[0], d[1] - a[1]);
    const [vx, vy] = [(d[0] - a[0]) / depth, (d[1] - a[1]) / depth];
    for (let s = 0, row = 0; s < depth - 0.5; s += 7, row++) {
      const t = Math.min(depth, s + 6.4);
      color.set(GROUND[crop]).multiplyScalar(row % 2 ? 0.94 : 1);
      const at = (p: Pt, m: number): Pt => [p[0] + vx * m, p[1] + vy * m];
      mb.poly([at(a, s), at(b, s), at(b, t), at(a, t)], 0, color);
    }
  }
  return mb.geometry();
}
