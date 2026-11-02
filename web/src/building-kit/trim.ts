// Trim: the plinth along the ground, window and door surrounds, and the cornice under flat
// roofs. A surround is one panel between the wall and the glass, costing two triangles.
import type * as THREE from 'three';
import type { MeshBuilder } from './mesh.ts';
import { along, type Edge, edge, type Pt } from './polygon.ts';

/** Windows and doors stand this far proud of the wall, and their frames half as far, so the
 * three layers never z-fight. */
export const PROUD = 0.08;
const FRAME_OUT = 0.04;
/** Plinth height on homes; doors start here, above a step. */
export const PLINTH = 0.3;
const PLINTH_OUT = 0.08;
const CORNICE = 0.35;
const CORNICE_OUT = 0.15;
/** Width of a window or door surround. */
const FRAME = 0.1;
const SILL_OUT = 0.16;

/**
 * A ledge from `y0` to `y1` standing `out` proud of every wall but `skip`, with a top and, if
 * `under`, a soffit. Each run reaches `out` past its ends, so square corners close.
 */
export function band(
  mb: MeshBuilder,
  ring: Pt[],
  y0: number,
  y1: number,
  out: number,
  col: THREE.Color,
  under = false,
  skip = -1,
) {
  for (let i = 0; i < ring.length; i++) {
    if (i === skip) continue;
    const e = edge(ring, i);
    if (e.L < 2 * out) continue;
    const [p, q] = [along(e, -out, out), along(e, e.L + out, out)];
    const [a, b] = [along(e, -out), along(e, e.L + out)];
    mb.wall(p, q, y0, y1, col, e.n);
    const ledge = (y: number, up: number) =>
      mb.quad([a[0], y, a[1]], [b[0], y, b[1]], [q[0], y, q[1]], [p[0], y, p[1]], col, [0, up, 0]);
    ledge(y1, 1);
    if (under) ledge(y0, -1);
  }
}

/** A plinth band `h` tall around the footprint, leaving out edge `skip`. */
export function plinth(mb: MeshBuilder, ring: Pt[], h: number, col: THREE.Color, skip = -1) {
  band(mb, ring, 0, h, PLINTH_OUT, col, false, skip);
}

/** A cornice band just under a flat roof at `h`, with its soffit. */
export function cornice(mb: MeshBuilder, ring: Pt[], h: number, col: THREE.Color) {
  band(mb, ring, h - CORNICE, h, CORNICE_OUT, col, true);
}

/** Room a surround needs above an opening, so it stays clear of a cornice or eave. */
export const HEAD_ROOM = CORNICE + FRAME;

/**
 * A frame panel just behind the opening from `s0` to `s1` and `y0` to `y1` on `e`, showing
 * as a border round it. With `sill`, a ledge juts out below it; without, it runs to `y0` (a door).
 */
export function surround(
  mb: MeshBuilder,
  e: Edge,
  s0: number,
  s1: number,
  y0: number,
  y1: number,
  col: THREE.Color,
  sill: boolean,
) {
  const bottom = sill ? y0 - FRAME : y0;
  mb.wall(
    along(e, s0 - FRAME, FRAME_OUT),
    along(e, s1 + FRAME, FRAME_OUT),
    bottom,
    y1 + FRAME,
    col,
    e.n,
  );
  if (!sill) return;
  const [a, b] = [along(e, s0 - FRAME * 1.5), along(e, s1 + FRAME * 1.5)];
  const [p, q] = [along(e, s0 - FRAME * 1.5, SILL_OUT), along(e, s1 + FRAME * 1.5, SILL_OUT)];
  mb.quad(
    [a[0], bottom, a[1]],
    [b[0], bottom, b[1]],
    [q[0], bottom, q[1]],
    [p[0], bottom, p[1]],
    col,
    [0, 1, 0],
  );
}

/** A step in front of a door centred `m` along `e`, reaching just above the plinth. */
export function step(mb: MeshBuilder, e: Edge, m: number, col: THREE.Color) {
  const rot = Math.atan2(-e.u[1], e.u[0]);
  mb.box(along(e, m, 0.35), 0, 1.6, PLINTH + 0.02, 0.7, rot, col);
}
