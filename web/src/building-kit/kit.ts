// The building grammar: every building is a sim footprint, height, use and front edge.
// Visual-only variety comes from a hash of the footprint, never from sim randomness.
import * as THREE from 'three';
import { balconies, door, loadingDoors, rooftop, shopfront, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import { at, type Box, clip, convex, obb, type Pt } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';
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
  /** Counter-clockwise footprint in scene-local (x, z). */
  ring: Pt[];
}

/** A small seeded generator (mulberry32) for one building's visual choices. */
export function seeded(ring: Pt[]): () => number {
  let a = 0x9e3779b9;
  for (const [x, z] of ring.slice(0, 2))
    a = Math.imul(a ^ Math.round(x * 16), 0x85ebca6b) ^ Math.imul(Math.round(z * 16), 0xc2b2ae35);
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (list: number[], r: number) => new THREE.Color(list[Math.floor(r * list.length)]);

export function building(mb: MeshBuilder, b: Building, style: Style) {
  const rand = seeded(b.ring);
  const box = obb(b.ring);
  const glass = new THREE.Color(style.glass);
  if (b.zone <= 2) (b.zone === 1 ? house : flats)(mb, b, box, style, rand, glass);
  else if (b.zone <= 4) shop(mb, b, box, style, rand, glass);
  else industry(mb, b, box, style, rand, glass);
}

type Part = (
  mb: MeshBuilder,
  b: Building,
  box: Box,
  style: Style,
  rand: () => number,
  glass: THREE.Color,
) => void;

const house: Part = (mb, b, box, style, rand, glass) => {
  const s = style.resLow;
  const wall = pick(s.walls, rand());
  const roof = pick(s.roofs, rand());
  const [form, pitch] = [
    rand() < s.gableShare ? 'gable' : 'hip',
    s.pitch[0] + rand() * (s.pitch[1] - s.pitch[0]),
  ] as const;
  const pitched = convex(b.ring);
  const top = pitched
    ? roofed(mb, b.ring, 0, roofPlanes(form, box, b.height, pitch), wall, roof, 0.45)
    : flatRoofed(mb, b.ring, 0, b.height, wall, roof, 0.4, wall);
  windows(
    mb,
    b.ring,
    top,
    { storey: 3, floors: Math.max(1, Math.round(b.height / 3)), h: 1.25 },
    glass,
  );
  door(mb, b.ring, b.front, 0.35 + rand() * 0.3, new THREE.Color(style.door));
  if (pitched && rand() < 0.65) {
    const c = at(box, box.L * (rand() - 0.5) * 0.8, 0);
    mb.box(
      c,
      top(c) - 1.2,
      0.8,
      2.3,
      0.8,
      Math.atan2(-box.u[1], box.u[0]),
      new THREE.Color(style.chimney),
    );
  }
};

const flats: Part = (mb, b, box, style, rand, glass) => {
  const s = style.resHigh;
  const wall = pick(s.walls, rand());
  const cap = new THREE.Color(s.parapet);
  const top = flatRoofed(mb, b.ring, 0, b.height, wall, new THREE.Color(s.roof), 0.7, cap);
  const floors = Math.floor(b.height / 3);
  windows(mb, b.ring, top, { storey: 3, floors, h: 1.4, w: 1.4, gap: 1.2 }, glass);
  // A stair and lift housing on the roof, kept inside the walls.
  rooftop(mb, b.ring, box, b.height, 1, true, rand, cap);
  if (rand() < 0.6) balconies(mb, b.ring, b.front, floors, cap);
};

const shop: Part = (mb, b, box, style, rand, glass) => {
  const high = b.zone === 4;
  const s = high ? style.comHigh : style.comLow;
  const wall = pick(s.walls, rand());
  const roof = new THREE.Color(s.roof);
  const cap = wall.clone().multiplyScalar(0.9);
  // Taller blocks on convex lots step back to a smaller top floor.
  const setback = high && b.height > 20 && rand() < 0.6 && convex(b.ring);
  const body = setback ? b.height - 4 : b.height;
  flatRoofed(mb, b.ring, 0, body, wall, roof, high ? 0.6 : 1, cap);
  if (setback) {
    const inner = clipBox(b.ring, box, 2.2);
    if (inner.length >= 3) flatRoofed(mb, inner, body, b.height, wall, roof, 0.5, cap);
  }
  const awning = !high || rand() < 0.5 ? pick(style.awnings, rand()) : null;
  shopfront(mb, b.ring, b.front, awning, style);
  const upper = { storey: 4, first: 4, floors: Math.floor(body / 4) - 1 };
  const rows = high
    ? { ...upper, sill: 0.6, h: 2.2, w: 2.6, gap: 0.6 }
    : { ...upper, h: 1.4, w: 1.3 };
  windows(mb, b.ring, () => body, rows, glass);
  rooftop(mb, b.ring, box, b.height, high ? 3 : 1, false, rand, new THREE.Color(style.rooftop));
};

/** The footprint pulled in by `d` on every side of its bounding box. */
function clipBox(ring: Pt[], box: Box, d: number): Pt[] {
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

const industry: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const variant = Math.min(Math.max(b.roof, 0), 3);
  const wall = pick(s.walls, rand());
  const roof = new THREE.Color(s.roofs[variant]);
  const h = b.height;
  const rect = b.ring.length === 4 && convex(b.ring);
  if (variant === 1 && rect) sawtooth(mb, b.ring, box, h, wall, roof, glass);
  else if (variant === 2 && rect)
    roofed(mb, b.ring, 0, roofPlanes('barrel', box, h), wall, roof, 0.3);
  else {
    flatRoofed(mb, b.ring, 0, h, wall, roof, 0.6, wall.clone().multiplyScalar(0.88));
    if (variant === 3) {
      const c = at(box, box.L * 0.6, 0);
      mb.cylinder(c, h, 1, 9 + rand() * 6, 8, new THREE.Color(s.stack));
      mb.cylinder(at(box, box.L * 0.6 - 4, 0), h, 2.2, 3.5, 8, new THREE.Color(style.rooftop));
    } else rooftop(mb, b.ring, box, h, 4, true, rand, new THREE.Color(style.rooftop));
  }
  loadingDoors(mb, b.ring, b.front, h, new THREE.Color(s.door));
  windows(
    mb,
    b.ring,
    () => h,
    { storey: h, floors: 1, sill: h - 2.4, h: 1, w: 2.4, gap: 2.2, skip: b.front },
    glass,
  );
};

/** North-light roof: sheds across the long axis, glazed on their tall faces. */
function sawtooth(
  mb: MeshBuilder,
  ring: Pt[],
  box: Box,
  h: number,
  wall: THREE.Color,
  roof: THREE.Color,
  glass: THREE.Color,
) {
  const n = Math.max(2, Math.round((box.L * 2) / 9));
  const step = (box.L * 2) / n;
  const across = (p: Pt) => (p[0] - box.c[0]) * box.u[0] + (p[1] - box.c[1]) * box.u[1];
  for (let k = 0; k < n; k++) {
    const lo = -box.L + k * step;
    const strip = clip(
      clip(ring, (p) => across(p) - lo),
      (p) => lo + step - across(p),
    );
    if (strip.length < 3) continue;
    const c = at(box, lo + step / 2, 0);
    roofed(
      mb,
      strip,
      0,
      [{ n: box.u, c, reach: step / 2, slope: 2.8 / step, base: h }],
      wall,
      roof,
    );
    const [p, q] = [at(box, lo + 0.02, -box.W + 0.6), at(box, lo + 0.02, box.W - 0.6)];
    mb.wall(p, q, h + 0.4, h + 2.4, glass, [-box.u[0], -box.u[1]]);
  }
}
