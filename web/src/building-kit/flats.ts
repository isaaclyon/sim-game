// Apartments: dense homes on the whole lot, with party walls to their neighbours. Only the
// front grows bays, galleries or dormers, so nothing reaches into the next building.
import * as THREE from 'three';
import { balconies, pane, rooftop, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import { type Archetype, clipBox, type Part, pick, roomFor, silo, trimFor, turn } from './parts.ts';
import { along, at, convex, edge, type Pt } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';
import type { Style } from './style.ts';
import { band, cornice, PROUD, plinth } from './trim.ts';

const floorsOf = (h: number) => Math.floor(h / 3);
const ROWS = { storey: 3, h: 1.4, w: 1.4, gap: 1.2 };

/** The plain flat-roofed block every design builds on; returns the wall colour. */
function block(
  mb: MeshBuilder,
  ring: Pt[],
  h: number,
  wall: THREE.Color,
  style: Style,
  glass: THREE.Color,
  parapet = 0.7,
): THREE.Color {
  const cap = new THREE.Color(style.resHigh.parapet);
  const top = flatRoofed(mb, ring, 0, h, wall, new THREE.Color(style.resHigh.roof), parapet, cap);
  plinth(mb, ring, 0.6, new THREE.Color(style.plinth));
  windows(mb, ring, top, { ...ROWS, floors: floorsOf(h) }, glass, trimFor(wall, style));
  return cap;
}

const slab: Part = (mb, b, box, style, rand, glass) => {
  const cap = block(mb, b.ring, b.height, pick(style.resHigh.walls, rand()), style, glass);
  // A stair and lift housing on the roof, kept inside the walls.
  rooftop(mb, b.ring, box, b.height, 1, true, rand, cap);
  if (rand() < 0.6) balconies(mb, b.ring, b.front, floorsOf(b.height), cap);
};

/** A slate mansard attic with dormers along the street. */
const mansard: Part = (mb, b, box, style, rand, glass) => {
  if (!convex(b.ring)) return slab(mb, b, box, style, rand, glass);
  const wall = pick(style.resHigh.walls, rand());
  const cap = new THREE.Color(style.resHigh.parapet);
  const top = roofed(
    mb,
    b.ring,
    0,
    roofPlanes('mansard', box, b.height, 14),
    wall,
    new THREE.Color(style.resLow.slate),
  );
  cornice(mb, b.ring, b.height, cap);
  plinth(mb, b.ring, 0.6, new THREE.Color(style.plinth));
  const rows = { storey: 3, floors: floorsOf(b.height), h: 1.6, w: 1.1, gap: 1.6 };
  windows(mb, b.ring, top, rows, glass, trimFor(wall, style));
  const e = edge(b.ring, b.front);
  const n = Math.floor((e.L - 2) / 3.2);
  const start = (e.L - (n - 1) * 3.2) / 2;
  for (let k = 0; k < n; k++) {
    const s = start + k * 3.2;
    mb.box(along(e, s, -0.75), b.height, 1.3, 2, 1.3, turn(e.u), wall);
    const [p, q] = [along(e, s - 0.4, -0.1 + PROUD), along(e, s + 0.4, -0.1 + PROUD)];
    pane(mb, p, q, b.height + 0.4, b.height + 1.6, glass, e.n, floorsOf(b.height));
  }
};

/** The top floor steps back behind a planted roof terrace. */
const terraced: Part = (mb, b, box, style, rand, glass) => {
  const inner = convex(b.ring) && box.W > 6 ? clipBox(b.ring, box, 2.5) : [];
  if (inner.length < 3) return slab(mb, b, box, style, rand, glass);
  const s = style.resHigh;
  const wall = pick(s.walls, rand());
  const cap = new THREE.Color(s.parapet);
  const body = b.height - 3;
  flatRoofed(mb, b.ring, 0, body, wall, new THREE.Color(s.garden), 1, cap);
  plinth(mb, b.ring, 0.6, new THREE.Color(style.plinth));
  windows(mb, b.ring, () => body, { ...ROWS, floors: floorsOf(body) }, glass, trimFor(wall, style));
  const light = wall.clone().lerp(new THREE.Color(0xffffff), 0.45);
  flatRoofed(mb, inner, body, b.height, light, new THREE.Color(s.roof), 0.4, cap);
  const rows = { storey: 3, first: body, floors: 1, sill: 0.3, h: 2, w: 2.2, gap: 0.8 };
  windows(mb, inner, () => b.height, rows, glass, null);
};

/** Brick with stone string courses at every floor and a water tank on legs. */
const brick: Part = (mb, b, box, style, rand, glass) => {
  const s = style.resHigh;
  const wall = pick(s.brick, rand());
  const stone = new THREE.Color(s.parapet);
  const floors = floorsOf(b.height);
  flatRoofed(mb, b.ring, 0, b.height, wall, new THREE.Color(s.roof), 0.9, stone);
  plinth(mb, b.ring, 0.9, stone);
  for (let f = 1; f < floors; f++) band(mb, b.ring, 3 * f - 0.2, 3 * f, 0.06, stone);
  const rows = { storey: 3, floors, sill: 0.7, h: 1.7, w: 1, gap: 1.5 };
  windows(mb, b.ring, () => b.height, rows, glass, stone);
  const c = at(box, -box.L * 0.45, 0);
  if (!roomFor(b.ring, c, 2.4)) return;
  const steel = new THREE.Color(style.industry.steel);
  const rot = turn(box.u);
  for (const [x, z] of [
    [-1, -1],
    [-1, 1],
    [1, 1],
    [1, -1],
  ])
    mb.box(at({ ...box, c }, x, z), b.height, 0.2, 2.2, 0.2, rot, steel);
  const tank = new THREE.Color(s.tank);
  silo(mb, c, b.height + 2.2, 1.5, 2.6, 0.9, tank, tank);
};

/** Deck access: an open walkway along the front of every upper floor, with a stair core. */
const gallery: Part = (mb, b, box, style, rand, glass) => {
  const e = edge(b.ring, b.front);
  if (e.L < 10) return slab(mb, b, box, style, rand, glass);
  const cap = block(mb, b.ring, b.height, pick(style.resHigh.walls, rand()), style, glass);
  const [s0, s1] = [0.3, e.L - 3];
  for (let f = 1; f < floorsOf(b.height); f++) {
    const y = 3 * f;
    const [a, q, c, d] = [along(e, s0), along(e, s1), along(e, s1, 1.5), along(e, s0, 1.5)];
    mb.quad([a[0], y, a[1]], [q[0], y, q[1]], [c[0], y, c[1]], [d[0], y, d[1]], cap, [0, 1, 0]);
    const below = y - 0.2;
    mb.quad(
      [a[0], below, a[1]],
      [q[0], below, q[1]],
      [c[0], below, c[1]],
      [d[0], below, d[1]],
      cap,
      [0, -1, 0],
    );
    mb.wall(d, c, below, y + 1.05, cap, e.n);
    mb.wall(a, d, below, y + 1.05, cap, [-e.u[0], -e.u[1]]);
  }
  const core = new THREE.Color(style.resHigh.parapet).multiplyScalar(0.92);
  mb.box(along(e, e.L - 1.6, 0.9), 0, 2.6, b.height + 1.2, 1.8, turn(e.u), core);
};

/** Bay windows stand out from the front on every upper floor. */
const bays: Part = (mb, b, _box, style, rand, glass) => {
  const wall = pick(style.resHigh.walls, rand());
  const cap = block(mb, b.ring, b.height, wall, style, glass);
  const e = edge(b.ring, b.front);
  const bay = wall.clone().multiplyScalar(0.94);
  for (let s = 2.2; s + 2.8 <= e.L - 1; s += 6.5) {
    mb.box(along(e, s + 1.4, 0.45), 3, 2.8, b.height - 3.4, 0.9, turn(e.u), bay);
    mb.box(along(e, s + 1.4, 0.5), b.height - 0.4, 3, 0.2, 1, turn(e.u), cap);
    for (let f = 1; f < floorsOf(b.height); f++) {
      const [p, q] = [along(e, s + 0.45, 0.9 + PROUD), along(e, s + 2.35, 0.9 + PROUD)];
      pane(mb, p, q, 3 * f + 0.8, 3 * f + 2.3, glass, e.n, f);
    }
  }
};

/** A walk-up under a tiled gable roof, with chimney stacks at both ends. */
const gabled: Part = (mb, b, box, style, rand, glass) => {
  if (!convex(b.ring)) return slab(mb, b, box, style, rand, glass);
  const wall = pick(style.resHigh.walls, rand());
  const roof = pick(style.resLow.roofs, rand());
  const pitch = Math.min(28, (Math.atan(5 / box.W) * 180) / Math.PI);
  const top = roofed(mb, b.ring, 0, roofPlanes('gable', box, b.height, pitch), wall, roof, 0.5);
  plinth(mb, b.ring, 0.6, new THREE.Color(style.plinth));
  windows(mb, b.ring, top, { ...ROWS, floors: floorsOf(b.height) }, glass, trimFor(wall, style));
  const brick = new THREE.Color(style.chimney);
  for (const side of [-1, 1]) {
    const c = at(box, side * (box.L - 1.2), 0);
    mb.box(c, top(c) - 1.5, 1.2, 3, 0.8, turn(box.u), brick);
  }
  if (rand() < 0.5)
    balconies(mb, b.ring, b.front, floorsOf(b.height), new THREE.Color(style.resHigh.parapet));
};

export const FLATS: Archetype[] = [
  { name: 'slab', draw: slab },
  { name: 'mansard', draw: mansard },
  { name: 'terraced', draw: terraced },
  { name: 'brick', draw: brick },
  { name: 'gallery', draw: gallery },
  { name: 'bays', draw: bays },
  { name: 'gabled', draw: gabled },
];
