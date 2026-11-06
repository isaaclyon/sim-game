// Heavy industry's own designs: big halls, tank farms, cement silos, process towers, stepped
// plants and storage domes. Each falls back to a simpler form where its footprint is too small
// or irregular. Nothing rises more than 9 m above the eaves, so a chimney stays the tallest
// thing and its smoke leaves from the top.
import * as THREE from 'three';
import { rooftop, windows } from './facade.ts';
import { finish, roofColour, shed, type Works, works } from './industry.ts';
import type { MeshBuilder } from './mesh.ts';
import {
  type Building,
  depthBehind,
  edgeFrom,
  frontPart,
  type Part,
  pick,
  roomFor,
  silo,
  turn,
} from './parts.ts';
import { along, clip, convex, edge, type Pt } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';

/** How far the footprint reaches behind its front edge. */
function depthOf(b: Building): number {
  const e = edge(b.ring, b.front);
  return Math.max(...b.ring.map((p) => depthBehind(e, p)));
}

/** A tall steel-mill hall: a low gable with a glazed lantern along the ridge. */
const hall: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.industry.metal, rand());
  if (!convex(b.ring)) return shed(mb, b, b.ring, b.height, wall, style, glass);
  const roof = roofColour(b, style);
  const pitch = Math.min(12, (Math.atan(4 / box.W) * 180) / Math.PI);
  const top = roofed(mb, b.ring, 0, roofPlanes('gable', box, b.height, pitch), wall, roof, 0.3);
  const ridge = top(box.c);
  mb.box(box.c, ridge - 0.4, box.L * 1.7, 1.8, 2.6, turn(box.u), glass);
  mb.box(box.c, ridge + 1.4, box.L * 1.7 + 0.4, 0.3, 3.6, turn(box.u), roof);
  finish(mb, b, b.ring, b.height, style, glass);
};

/** A tank farm: a process shed in front, squat storage tanks behind and a pipe rack between. */
const tanks: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const e = edge(b.ring, b.front);
  const depth = depthOf(b);
  const keep = depth * 0.55;
  const r = Math.min((depth - keep) / 2 - 0.5, 6);
  if (r < 2.5 || !convex(b.ring)) return hall(mb, b, box, style, rand, glass);
  shed(mb, b, frontPart(b, keep), b.height, pick(s.walls, rand()), style, glass);
  const n = Math.max(1, Math.min(4, Math.floor((e.L - 1) / (2 * r + 1.5))));
  const white = pick(s.silo, rand());
  for (let k = 0; k < n; k++) {
    const c = along(e, e.L / 2 + (k - (n - 1) / 2) * (2 * r + 1.5), -(keep + (depth - keep) / 2));
    if (roomFor(b.ring, c, r))
      silo(mb, c, 0, r, 3 + r * 0.8, 0.6, white, white.clone().multiplyScalar(0.92));
  }
  const steel = new THREE.Color(s.steel);
  const rack = -(keep + 0.35);
  mb.box(along(e, e.L / 2, rack), 4.5, e.L * 0.85, 0.5, 0.6, turn(e.u), steel);
  for (let a = e.L * 0.075 + 0.3; a < e.L * 0.925; a += 5)
    mb.box(along(e, a, rack), 0, 0.3, 4.5, 0.3, turn(e.u), steel);
};

/** Cement works: a row of tall silos behind the shed, fed by a gallery from a tower. */
const cement: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const e = edge(b.ring, b.front);
  const depth = depthOf(b);
  const keep = depth * 0.6;
  const r = Math.min((depth - keep) / 2 - 0.4, 3.5);
  if (r < 2 || !convex(b.ring)) return hall(mb, b, box, style, rand, glass);
  shed(mb, b, frontPart(b, keep), b.height, pick(s.walls, rand()), style, glass);
  const concrete = pick(s.silo, rand());
  const row = -(keep + (depth - keep) / 2);
  const pitch = 2 * r + 0.3;
  const first = 1 + r;
  const n = Math.max(1, Math.min(6, Math.floor((e.L - first - r - 4.5) / pitch) + 1));
  const top = b.height + 6;
  for (let k = 0; k < n; k++) {
    const c = along(e, first + k * pitch, row);
    if (roomFor(b.ring, c, r))
      silo(mb, c, 0, r, top, r * 0.4, concrete, concrete.clone().multiplyScalar(0.9));
  }
  const at = first + (n - 1) * pitch + r + 1.8;
  const t = along(e, at, row);
  if (!roomFor(b.ring, t, 2.2)) return;
  mb.box(t, 0, 3, top + 2.5, 3, turn(e.u), concrete.clone().multiplyScalar(0.85));
  mb.box(
    along(e, (first + at) / 2, row),
    top + 1.2,
    at - first,
    1.2,
    1.4,
    turn(e.u),
    new THREE.Color(s.steel),
  );
};

/** A process tower with stacked platforms rising beside a flat-roofed plant. */
const tower: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const wall = pick(s.walls, rand());
  shed(mb, b, b.ring, b.height, wall, style, glass);
  rooftop(mb, b.ring, box, b.height, 3, true, rand, new THREE.Color(style.rooftop));
  const e = edge(b.ring, b.front);
  const c = along(e, e.L - 5, -depthOf(b) * 0.55);
  if (!roomFor(b.ring, c, 4.5)) return;
  const rot = turn(e.u);
  const steel = new THREE.Color(s.steel);
  mb.box(c, 0, 5, b.height + 8, 5, rot, wall.clone().multiplyScalar(0.8));
  for (let y = b.height + 1.5; y < b.height + 8; y += 3) mb.box(c, y, 6.2, 0.25, 6.2, rot, steel);
  mb.cylinder(along(e, e.L - 8, -depthOf(b) * 0.55), 0, 0.35, b.height + 7, 6, steel);
};

/** A plant that steps up away from the street in three blocks. */
const stepped: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const wall = pick(s.walls, rand());
  if (!convex(b.ring)) return shed(mb, b, b.ring, b.height, wall, style, glass);
  const e = edge(b.ring, b.front);
  const depth = depthOf(b);
  const slice = (d0: number, d1: number) =>
    clip(
      clip(b.ring, (p) => depthBehind(e, p) - d0),
      (p) => d1 - depthBehind(e, p),
    );
  const low = Math.max(4, b.height * 0.6);
  const roof = roofColour(b, style);
  const cap = wall.clone().multiplyScalar(0.88);
  const blocks: [Pt[], number][] = [
    [slice(-1, depth / 3), low],
    [slice(depth / 3, (2 * depth) / 3), b.height],
    [slice((2 * depth) / 3, depth + 1), b.height + 4.5],
  ];
  for (const [ring, h] of blocks) {
    if (ring.length < 3) continue;
    flatRoofed(mb, ring, 0, h, wall, roof, 0.5, cap);
    if (h === low) finish(mb, b, ring, h, style, glass);
    else {
      const rows = { storey: h, floors: 1, sill: h - 2.4, h: 1, w: 2.4, gap: 2.2 };
      windows(mb, ring, () => h, { ...rows, skip: edgeFrom(ring, b.ring[b.front]) }, glass, null);
    }
  }
  if (blocks[2][0].length >= 3)
    rooftop(mb, blocks[2][0], box, b.height + 4.5, 2, true, rand, new THREE.Color(style.rooftop));
};

/** A bulk store: a shed in front of one to three low storage domes. */
const domes: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const e = edge(b.ring, b.front);
  const depth = depthOf(b);
  const keep = depth * 0.5;
  const r = Math.min((depth - keep) / 2 - 0.4, 7);
  if (r < 3 || !convex(b.ring)) return hall(mb, b, box, style, rand, glass);
  shed(mb, b, frontPart(b, keep), b.height, pick(s.walls, rand()), style, glass);
  const n = Math.max(1, Math.min(3, Math.floor((e.L - 1) / (2 * r + 1))));
  const col = pick(s.silo, rand());
  for (let k = 0; k < n; k++) {
    const c = along(e, e.L / 2 + (k - (n - 1) / 2) * (2 * r + 1), -(keep + (depth - keep) / 2));
    if (roomFor(b.ring, c, r)) dome(mb, c, r, col);
  }
};

/** A low-poly dome of radius `r`: stacked tapering rings and a cone on top. */
function dome(mb: MeshBuilder, c: Pt, r: number, col: THREE.Color) {
  const rings = [
    [0, 1],
    [0.4, 0.92],
    [0.7, 0.7],
    [0.9, 0.4],
  ];
  for (let i = 0; i < rings.length - 1; i++) {
    const [[y0, r0], [y1, r1]] = [rings[i], rings[i + 1]];
    mb.cylinder(c, r * y0, r * r0, r * (y1 - y0), 14, col, r * r1);
  }
  mb.cylinder(c, r * 0.9, r * 0.4, r * 0.1, 14, col, 0);
}

export const FACTORIES: Works[] = [
  works('hall', hall),
  works('tanks', tanks),
  works('cement', cement),
  works('tower', tower),
  works('stepped', stepped),
  works('domes', domes),
];
