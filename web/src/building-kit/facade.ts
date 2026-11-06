// Wall and roof details: windows, shopfronts, doors, balconies and rooftop units.
import * as THREE from 'three';
import type { MeshBuilder } from './mesh.ts';
import { along, type Box, edge, inside, type Pt } from './polygon.ts';
import type { Roof } from './roof.ts';
import type { Style } from './style.ts';
import { HEAD_ROOM, PLINTH, PROUD, step, surround } from './trim.ts';

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
  /** A door `at` metres along `edge`; ground-floor windows give way to it. */
  door?: { edge: number; at: number };
}

/** Rows of windows on every wall long enough, kept below the roof line; framed with sills
 * unless `frame` is null. */
export function windows(
  mb: MeshBuilder,
  ring: Pt[],
  top: Roof,
  rows: WindowRows,
  glass: THREE.Color,
  frame: THREE.Color | null,
) {
  const {
    storey,
    floors,
    first = 0,
    sill = 0.9,
    h = 1.3,
    w = 1.2,
    gap = 1.4,
    skip = -1,
    door,
  } = rows;
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
        if (f === 0 && door?.edge === i && Math.abs(s + w / 2 - door.at) < w / 2 + 0.8) continue;
        const [p, q] = [along(e, s, PROUD), along(e, s + w, PROUD)];
        if (top(p) < y1 + HEAD_ROOM || top(q) < y1 + HEAD_ROOM) continue;
        pane(mb, p, q, y0, y1, glass, e.n, f);
        if (frame) surround(mb, e, s, s + w, y0, y1, frame, true);
      }
    }
  }
}

/** One window pane from `p` to `q` on floor `f`; some panes light up at night. */
export function pane(
  mb: MeshBuilder,
  p: Pt,
  q: Pt,
  y0: number,
  y1: number,
  glass: THREE.Color,
  n: Pt,
  f: number,
) {
  // Stable variation in which windows are lit, independent of gameplay RNG.
  if (Math.sin(p[0] * 12.9898 + p[1] * 78.233 + f * 37.7) > -0.25) mb.glow.set(0xffcf83);
  mb.wall(p, q, y0, y1, glass, n);
  mb.glow.set(0x000000);
}

/** Ground-floor glazing along the front, with an awning on small shops; with `stripes`, the
 * awning alternates with that colour every metre or so. */
export function shopfront(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  awning: THREE.Color | null,
  style: Style,
  entrance: number | null = null,
  stripes: THREE.Color | null = null,
) {
  const e = edge(ring, front);
  if (e.L < 3) return;
  mb.glow.set(0xffdfab);
  const panes =
    entrance === null
      ? [[0.6, e.L - 0.6]]
      : [
          [0.6, entrance - 0.7],
          [entrance + 0.7, e.L - 0.6],
        ];
  for (const [start, end] of panes) {
    if (end <= start) continue;
    mb.wall(
      along(e, start, PROUD),
      along(e, end, PROUD),
      0.35,
      3.1,
      new THREE.Color(style.shopGlass),
      e.n,
    );
  }
  mb.glow.set(0x000000);
  if (!awning) return;
  const n = stripes ? Math.max(1, Math.round((e.L - 0.8) / 1.1)) : 1;
  const w = (e.L - 0.8) / n;
  for (let k = 0; k < n; k++) {
    const col = stripes && k % 2 ? stripes : awning;
    const [s0, s1] = [0.4 + k * w, 0.4 + (k + 1) * w];
    const [a, b] = [along(e, s0, PROUD), along(e, s1, PROUD)];
    const [c, d] = [along(e, s1, 1.7), along(e, s0, 1.7)];
    mb.quad([a[0], 3.45, a[1]], [b[0], 3.45, b[1]], [c[0], 2.85, c[1]], [d[0], 2.85, d[1]], col, [
      e.n[0],
      1,
      e.n[1],
    ]);
    mb.wall(d, c, 2.5, 2.85, col.clone().multiplyScalar(0.85), e.n);
  }
}

/** A framed front door `m` metres along the front edge, up a step from the plinth. */
export function door(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  m: number,
  col: THREE.Color,
  frame: THREE.Color,
  plinth: THREE.Color,
) {
  const e = edge(ring, front);
  if (e.L < 4) return;
  mb.wall(along(e, m - 0.5, PROUD), along(e, m + 0.5, PROUD), PLINTH, PLINTH + 2.1, col, e.n);
  surround(mb, e, m - 0.5, m + 0.5, PLINTH, PLINTH + 2.1, frame, false);
  step(mb, e, m, plinth);
}

/** Large doors every 7 m along an industrial front; several colours take turns. */
export function loadingDoors(
  mb: MeshBuilder,
  ring: Pt[],
  front: number,
  h: number,
  col: THREE.Color | THREE.Color[],
) {
  const e = edge(ring, front);
  const top = Math.min(4.6, h - 1.2);
  const cols = Array.isArray(col) ? col : [col];
  for (let k = 0; k < Math.floor((e.L - 4) / 7); k++) {
    const s = 3 + k * 7;
    mb.wall(along(e, s, PROUD), along(e, s + 4, PROUD), 0, top, cols[k % cols.length], e.n);
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
