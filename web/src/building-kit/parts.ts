// Shared pieces of the building grammar: the record, seeded choices, colours and the footprint
// helpers every use's designs draw with.
import * as THREE from 'three';
import type { MeshBuilder } from './mesh.ts';
import { type Box, clip, type Edge, edge, inside, type Pt } from './polygon.ts';
import type { Style } from './style.ts';

export interface Building {
  /** `Zone` value: 1/2 residential, 3/4 commercial, 5/6 industrial, low then high density. */
  zone: number;
  /** Eave or roof height in metres, from the sim. */
  height: number;
  /** The sim's stable roof choice, 0–3 for industry. */
  roof: number;
  /** Index of the footprint edge facing the road. */
  front: number;
  /** The era it went up in, kept for life (ADR 0059): 0 = village, 1 = town. Picks the look. */
  era: number;
  /** Counter-clockwise footprint in scene-local (x, z). */
  ring: Pt[];
}

/** A small seeded generator (mulberry32) for one building's visual choices. */
export function seeded(ring: Pt[], salt = 0): () => number {
  let a = 0x9e3779b9 ^ salt;
  for (const [x, z] of ring.slice(0, 2))
    a = Math.imul(a ^ Math.round(x * 16), 0x85ebca6b) ^ Math.imul(Math.round(z * 16), 0xc2b2ae35);
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export type Part = (
  mb: MeshBuilder,
  b: Building,
  box: Box,
  style: Style,
  rand: () => number,
  glass: THREE.Color,
) => void;

/** One named design for a use. The name is for tests and debugging only. */
export interface Archetype {
  name: string;
  draw: Part;
}

export const pick = (list: number[], r: number) =>
  new THREE.Color(list[Math.floor(r * list.length)]);

/** Surround colour that stands out from `wall`. */
export function trimFor(wall: THREE.Color, style: Style): THREE.Color {
  const { l } = wall.getHSL({ h: 0, s: 0, l: 0 });
  return new THREE.Color(l > 0.78 ? style.trim.dark : style.trim.light);
}

/** Rotation for `MeshBuilder.box` that lines a box's x axis up with direction `u`. */
export const turn = (u: Pt) => Math.atan2(-u[1], u[0]);

/** The footprint pulled in by `d` on every side of its bounding box. */
export function clipBox(ring: Pt[], box: Box, d: number): Pt[] {
  let out = ring;
  for (const [n, reach] of [
    [box.u, box.L],
    [[-box.u[0], -box.u[1]], box.L],
    [box.v, box.W],
    [[-box.v[0], -box.v[1]], box.W],
  ] as [Pt, number][])
    out = clip(out, (p) => reach - d - ((p[0] - box.c[0]) * n[0] + (p[1] - box.c[1]) * n[1]));
  return out;
}

/** The same box with its axes swapped, so a gable ridge runs across instead of along it. */
export function turned(box: Box): Box {
  const { c, u, v, L, W } = box;
  return { c, u: v, v: [-u[0], -u[1]], L: W, W: L };
}

/** The box turned so that `v` points out of the front edge and `u` runs along it. */
export function frontBox(b: Building, box: Box): Box {
  const n = edge(b.ring, b.front).n;
  let best = box;
  let next = box;
  for (let k = 0; k < 3; k++) {
    next = turned(next);
    if (next.v[0] * n[0] + next.v[1] * n[1] > best.v[0] * n[0] + best.v[1] * n[1]) best = next;
  }
  return best;
}

/** Metres behind the front edge `e`. */
export const depthBehind = (e: Edge, p: Pt) =>
  -((p[0] - e.a[0]) * e.n[0] + (p[1] - e.a[1]) * e.n[1]);

/** The part of the footprint within `d` metres of its front edge. */
export function frontPart(b: Building, d: number): Pt[] {
  const e = edge(b.ring, b.front);
  return clip(b.ring, (p) => d - depthBehind(e, p));
}

/** Index of the edge of `ring` that starts at `a`, or -1. Clipping renumbers edges. */
export function edgeFrom(ring: Pt[], a: Pt): number {
  return ring.findIndex((p) => Math.hypot(p[0] - a[0], p[1] - a[1]) < 1e-4);
}

/** Whether a round thing of radius `r` at `c` stands clear inside `ring`. */
export function roomFor(ring: Pt[], c: Pt, r: number): boolean {
  return [0, 1, 2, 3, 4, 5, 6, 7].every((i) => {
    const t = (i * Math.PI) / 4;
    return inside([c[0] + Math.cos(t) * r, c[1] + Math.sin(t) * r], ring);
  });
}

/** A cylinder topped by a cone, such as a silo or a tank, standing on `y0`. */
export function silo(
  mb: MeshBuilder,
  at: Pt,
  y0: number,
  r: number,
  h: number,
  cone: number,
  body: THREE.Color,
  cap: THREE.Color,
) {
  mb.cylinder(at, y0, r, h, 12, body);
  if (cone > 0) mb.cylinder(at, y0 + h, r, cone, 12, cap, 0);
}
