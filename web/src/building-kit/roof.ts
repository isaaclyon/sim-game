// A roof is the lowest of a few planes over the footprint; walls rise to meet it.
import type * as THREE from 'three';
import type { MeshBuilder } from './mesh.ts';
import { type Box, clip, offset, type Pt } from './polygon.ts';

/** Height `base + slope * (reach - distance along n from c)`. */
interface Plane {
  n: Pt;
  c: Pt;
  reach: number;
  slope: number;
  base: number;
}

export type RoofForm = 'flat' | 'gable' | 'hip' | 'barrel' | 'shed';

const height = (pl: Plane, p: Pt) =>
  pl.base + pl.slope * (pl.reach - ((p[0] - pl.c[0]) * pl.n[0] + (p[1] - pl.c[1]) * pl.n[1]));

export type Roof = (p: Pt) => number;

export function roofPlanes(form: RoofForm, box: Box, eave: number, pitchDeg = 0): Plane[] {
  const s = Math.tan((pitchDeg * Math.PI) / 180);
  const { u, v, L, W, c } = box;
  const plane = (n: Pt, reach: number, slope = s, base = eave): Plane => ({
    n,
    c,
    reach,
    slope,
    base,
  });
  const back: Pt = [-v[0], -v[1]];
  switch (form) {
    case 'flat':
      return [plane(u, 0, 0)];
    case 'shed':
      return [plane(v, W)];
    case 'gable':
      return [plane(v, W), plane(back, W)];
    case 'hip':
      return [plane(v, W), plane(back, W), plane(u, L), plane([-u[0], -u[1]], L)];
    case 'barrel': {
      // Three tangent planes per side approximate a shallow curve.
      const rise = Math.min(3.2, W * 0.22);
      const k = rise / (W * W);
      return [v, back].flatMap((n) =>
        [0.18, 0.5, 0.82].map((f) => {
          const d = f * W;
          return { n, c, reach: d, slope: 2 * k * d, base: eave + k * (W * W - d * d) };
        }),
      );
    }
  }
}

export const roofAt = (planes: Plane[], p: Pt) =>
  planes.reduce((m, pl) => Math.min(m, height(pl, p)), Infinity);

/**
 * Walls from `base` up to the roof, then the roof faces. Pitched roofs overhang by `overhang`
 * metres; they are only used on convex footprints. Returns the roof height function.
 */
export function roofed(
  mb: MeshBuilder,
  ring: Pt[],
  base: number,
  planes: Plane[],
  wall: THREE.Color,
  roof: THREE.Color,
  overhang = 0,
): Roof {
  const top = (p: Pt) => roofAt(planes, p);
  for (let i = 0; i < ring.length; i++) {
    const [a, b] = [ring[i], ring[(i + 1) % ring.length]];
    // Split each wall where the roof changes plane, so gables get their triangle.
    const ts = [0, 1];
    for (let j = 0; j < planes.length; j++)
      for (let k = j + 1; k < planes.length; k++) {
        const [j0, j1] = [height(planes[j], a), height(planes[j], b)];
        const [k0, k1] = [height(planes[k], a), height(planes[k], b)];
        const den = j1 - j0 - (k1 - k0);
        if (Math.abs(den) < 1e-9) continue;
        const t = (k0 - j0) / den;
        if (t > 0.001 && t < 0.999) ts.push(t);
      }
    ts.sort((x, y) => x - y);
    const n: Pt = [b[1] - a[1], -(b[0] - a[0])];
    const lerp = (t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    for (let q = 0; q < ts.length - 1; q++) {
      const [p0, p1] = [lerp(ts[q]), lerp(ts[q + 1])];
      mb.quad(
        [p0[0], base, p0[1]],
        [p1[0], base, p1[1]],
        [p1[0], top(p1), p1[1]],
        [p0[0], top(p0), p0[1]],
        wall,
        [n[0], 0, n[1]],
      );
    }
  }
  const eaves = overhang ? offset(ring, overhang) : ring;
  for (let k = 0; k < planes.length; k++) {
    let face = eaves;
    for (let j = 0; j < planes.length && face.length >= 3; j++)
      if (j !== k) face = clip(face, (p) => height(planes[j], p) - height(planes[k], p));
    if (face.length >= 3) mb.poly(face, (p) => height(planes[k], p), roof);
  }
  return top;
}

/** A flat roof at `h` behind a parapet band of `parapet` metres. Safe on concave rings. */
export function flatRoofed(
  mb: MeshBuilder,
  ring: Pt[],
  base: number,
  h: number,
  wall: THREE.Color,
  roof: THREE.Color,
  parapet: number,
  cap: THREE.Color,
): Roof {
  for (let i = 0; i < ring.length; i++) {
    const [p, q] = [ring[i], ring[(i + 1) % ring.length]];
    const n: Pt = [q[1] - p[1], -(q[0] - p[0])];
    mb.wall(p, q, base, h, wall, n);
    if (parapet > 0) mb.wall(p, q, h, h + parapet, cap, n);
  }
  mb.poly(ring, h, roof);
  return () => h;
}
