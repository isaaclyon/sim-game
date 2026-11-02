// Wall and roof details: windows, shopfronts, doors, balconies and rooftop units.
import * as THREE from 'three';
import type { MeshBuilder } from './mesh.ts';
import { along, type Box, edge, inside, type Pt } from './polygon.ts';
import type { Roof } from './roof.ts';
import type { Style } from './style.ts';

/** Windows stand this far proud of the wall so they never z-fight with it. */
const PROUD = 0.06;

export interface WindowRows {
  storey: number;
  floors: number;
  /** Height of the first window row's floor; shops put their glazing below it. */
  first?: number;
  sill?: number;
  h?: number;
  w?: number;
  gap?: number;
  /** An edge left blank, such as the industrial loading front. */
  skip?: number;
}

/** Rows of window quads on every wall long enough, kept below the roof line. */
export function windows(
  mb: MeshBuilder,
  ring: Pt[],
  top: Roof,
  rows: WindowRows,
  glass: THREE.Color,
) {
  const { storey, floors, first = 0, sill = 0.9, h = 1.3, w = 1.2, gap = 1.4, skip = -1 } = rows;
  for (let i = 0; i < ring.length; i++) {
    if (i === skip) continue;
    const e = edge(ring, i);
    if (e.L < 3.2) continue;
    const count = Math.max(1, Math.floor((e.L - 1) / (w + gap)));
    const start = (e.L - (count * w + (count - 1) * gap)) / 2;
    for (let f = 0; f < floors; f++) {
      const y0 = first + f * storey + sill;
      const y1 = y0 + h;
      for (let k = 0; k < count; k++) {
        const s = start + k * (w + gap);
        const [p, q] = [along(e, s, PROUD), along(e, s + w, PROUD)];
        if (top(p) < y1 + 0.35 || top(q) < y1 + 0.35) continue;
        mb.wall(p, q, y0, y1, glass, e.n);
      }
    }
  }
}

/** Ground-floor glazing along the front, with an awning on small shops. */
export function shopfront(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  awning: THREE.Color | null,
  style: Style,
) {
  const e = edge(ring, front);
  if (e.L < 3) return;
  mb.wall(
    along(e, 0.6, PROUD),
    along(e, e.L - 0.6, PROUD),
    0.35,
    3.1,
    new THREE.Color(style.shopGlass),
    e.n,
  );
  if (!awning) return;
  const [a, b] = [along(e, 0.4, PROUD), along(e, e.L - 0.4, PROUD)];
  const [c, d] = [along(e, e.L - 0.4, 1.7), along(e, 0.4, 1.7)];
  mb.quad([a[0], 3.45, a[1]], [b[0], 3.45, b[1]], [c[0], 2.85, c[1]], [d[0], 2.85, d[1]], awning, [
    e.n[0],
    1,
    e.n[1],
  ]);
  mb.wall(d, c, 2.5, 2.85, awning.clone().multiplyScalar(0.85), e.n);
}

/** A front door at `t` (0–1) along the front edge. */
export function door(mb: MeshBuilder, ring: Pt[], front: number, t: number, col: THREE.Color) {
  const e = edge(ring, front);
  if (e.L < 4) return;
  const m = e.L * t;
  mb.wall(along(e, m - 0.5, PROUD), along(e, m + 0.5, PROUD), 0, 2.1, col, e.n);
}

/** Large doors every 7 m along an industrial front. */
export function loadingDoors(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  h: number,
  col: THREE.Color,
) {
  const e = edge(ring, front);
  const top = Math.min(4.6, h - 1.2);
  for (let k = 0; k < Math.floor((e.L - 4) / 7); k++) {
    const s = 3 + k * 7;
    mb.wall(along(e, s, PROUD), along(e, s + 4, PROUD), 0, top, col, e.n);
  }
}

/** Balcony slabs with solid fronts, three wide, on every floor above the ground. */
export function balconies(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  floors: number,
  col: THREE.Color,
) {
  const e = edge(ring, front);
  if (e.L < 8) return;
  for (let f = 1; f < floors; f++) {
    const y = f * 3;
    for (let s = 2; s + 4 < e.L; s += 6) {
      const [a, b] = [along(e, s), along(e, s + 3)];
      const [c, d] = [along(e, s + 3, 1.2), along(e, s, 1.2)];
      mb.quad([a[0], y, a[1]], [b[0], y, b[1]], [c[0], y, c[1]], [d[0], y, d[1]], col, [0, 1, 0]);
      mb.wall(d, c, y, y + 1, col, e.n);
    }
  }
}

/** Up to `n` plant boxes on a flat roof at `h`, kept 2.5 m inside the walls. */
export function rooftop(
  mb: MeshBuilder,
  ring: Pt[],
  box: Box,
  h: number,
  n: number,
  big: boolean,
  rand: () => number,
  col: THREE.Color,
) {
  const rot = Math.atan2(-box.u[1], box.u[0]);
  for (let k = 0; k < n; k++) {
    const [a, b] = [(rand() - 0.5) * box.L * 1.2, (rand() - 0.5) * box.W];
    const sx = (big ? 3 : 1.8) + rand() * 2.2;
    const sz = 1.6 + rand() * 1.6;
    const sy = 1.1 + rand() * 1.3;
    const c: Pt = [box.c[0] + box.u[0] * a + box.v[0] * b, box.c[1] + box.u[1] * a + box.v[1] * b];
    const r = Math.hypot(sx, sz) / 2 + 2.5;
    const clear = [0, 1, 2, 3].every((i) => {
      const t = (i * Math.PI) / 2;
      return inside([c[0] + Math.cos(t) * r, c[1] + Math.sin(t) * r], ring);
    });
    if (clear) mb.box(c, h, sx, sy, sz, rot, col);
  }
}
