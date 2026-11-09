// Industry: sheds with loading doors along the front, where the lot's truck court meets them.
// The sim's roof choice (0–3) picks the roof colour and, for 3, a smoking chimney; the classic
// design also takes its roof form from it. Light industry (workshops) is in this file;
// heavy industry's own designs are in `factories.ts`.
import * as THREE from 'three';
import { loadingDoors, rooftop, type WindowRows, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import {
  type Archetype,
  type Building,
  depthBehind,
  edgeFrom,
  frontPart,
  type Part,
  pick,
  roomFor,
  seeded,
  silo,
  turn,
} from './parts.ts';
import { along, at, type Box, clip, convex, edge, type Pt } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';
import type { Style } from './style.ts';
import { PROUD, plinth } from './trim.ts';

export interface Smokestack {
  /** Centre in scene-local (x, z). */
  at: Pt;
  /** Height of the open top, metres. */
  top: number;
}

/** An industrial design and where it puts a chimney when the sim gives it one. */
export interface Works extends Archetype {
  stack: (b: Building, box: Box) => Smokestack;
}

export const roofIndex = (b: Building) => Math.min(Math.max(Math.round(b.roof), 0), 3);
export const hasStack = (b: Building) => roofIndex(b) === 3;
export const roofColour = (b: Building, style: Style) =>
  new THREE.Color(style.industry.roofs[roofIndex(b)]);

const STACK_SEED = 0x57ac;

/** New designs keep their chimney near one end of the front, inside the walls, and taller
 * than anything else they draw (at most 9 m above the eaves). */
function sideStack(b: Building): Smokestack {
  const e = edge(b.ring, b.front);
  return {
    at: along(e, Math.min(4, e.L / 2), -3),
    top: b.height + 10 + seeded(b.ring, STACK_SEED)() * 6,
  };
}

/** A tapering chimney from the ground, with a dark band near its top. */
function chimney(mb: MeshBuilder, s: Smokestack, style: Style) {
  mb.cylinder(s.at, 0, 1.25, s.top, 10, new THREE.Color(style.industry.stack), 0.95);
  mb.cylinder(s.at, s.top - 1.6, 1.12, 0.8, 10, new THREE.Color(style.industry.steel));
}

/** Wraps a design so it draws its chimney when the sim's roof choice asks for one. */
export function works(name: string, draw: Part, stack: Works['stack'] = sideStack): Works {
  return {
    name,
    stack,
    draw: (mb, b, box, style, rand, glass) => {
      draw(mb, b, box, style, rand, glass);
      if (hasStack(b)) chimney(mb, stack(b, box), style);
    },
  };
}

/**
 * Loading doors, plinth and high windows for a shed of height `h` standing on `ring`, all or
 * the front part of the footprint. `rows` replaces the high window strip.
 */
export function finish(
  mb: MeshBuilder,
  b: Building,
  ring: Pt[],
  h: number,
  style: Style,
  glass: THREE.Color,
  rows: Partial<WindowRows> = {},
  doors: THREE.Color | THREE.Color[] = new THREE.Color(style.industry.door),
) {
  loadingDoors(mb, b.ring, b.front, h, doors);
  const front = edgeFrom(ring, b.ring[b.front]);
  plinth(mb, ring, 0.5, new THREE.Color(style.plinth), front);
  const strip = { storey: h, floors: 1, sill: h - 2.4, h: 1, w: 2.4, gap: 2.2 };
  windows(mb, ring, () => h, { ...strip, ...rows, skip: front }, glass, null);
}

/** A plain flat-roofed shed: the fallback for footprints a design cannot shape. */
export function shed(
  mb: MeshBuilder,
  b: Building,
  ring: Pt[],
  h: number,
  wall: THREE.Color,
  style: Style,
  glass: THREE.Color,
) {
  flatRoofed(mb, ring, 0, h, wall, roofColour(b, style), 0.6, wall.clone().multiplyScalar(0.88));
  finish(mb, b, ring, h, style, glass);
}

/** The classic chimney: the sim's roof choice 3 on the flat-roofed classic shed. */
function classicStack(b: Building, box: Box): Smokestack {
  const rand = seeded(b.ring);
  rand(); // wall colour
  return { at: at(box, box.L * 0.6, 0), top: b.height + 9 + rand() * 6 };
}

/** The original shed: flat with rooftop plant, sawtooth, barrel or chimney by the sim's roof. */
const classicDraw: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const variant = roofIndex(b);
  const wall = pick(s.walls, rand());
  const roof = roofColour(b, style);
  const h = b.height;
  const rect = b.ring.length === 4 && convex(b.ring);
  if (variant === 1 && rect) sawtooth(mb, b.ring, box, h, wall, roof, glass);
  else if (variant === 2 && rect)
    roofed(mb, b.ring, 0, roofPlanes('barrel', box, h), wall, roof, 0.3);
  else {
    flatRoofed(mb, b.ring, 0, h, wall, roof, 0.6, wall.clone().multiplyScalar(0.88));
    if (variant === 3) {
      const stack = classicStack(b, box);
      mb.cylinder(stack.at, h, 1, stack.top - h, 8, new THREE.Color(s.stack));
      mb.cylinder(at(box, box.L * 0.6 - 4, 0), h, 2.2, 3.5, 8, new THREE.Color(style.rooftop));
    } else rooftop(mb, b.ring, box, h, 4, true, rand, new THREE.Color(style.rooftop));
  }
  finish(mb, b, b.ring, h, style, glass);
};

export const classic: Works = { name: 'classic', draw: classicDraw, stack: classicStack };

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
    // The glass stands proud of the tall face, outside the strip, so the wall cannot show through.
    const [p, q] = [at(box, lo - PROUD, -box.W + 0.6), at(box, lo - PROUD, box.W - 0.6)];
    mb.wall(p, q, h + 0.4, h + 2.4, glass, [-box.u[0], -box.u[1]]);
  }
}

/** Glass roof lights `w` metres across, every 6 m along both slopes of a gable over `box`. */
function roofLights(
  mb: MeshBuilder,
  box: Box,
  top: (p: Pt) => number,
  w: number,
  glass: THREE.Color,
) {
  const n = Math.floor((box.L * 2) / 6);
  for (const side of [-1, 1])
    for (let k = 0; k < n; k++) {
      const a = -box.L + 3 + k * 6;
      const [b0, b1] = [side * box.W * 0.5 - w / 2, side * box.W * 0.5 + w / 2];
      const rect = [at(box, a - 1, b0), at(box, a + 1, b0), at(box, a + 1, b1), at(box, a - 1, b1)];
      mb.poly(rect, (p) => top(p) + 0.05, glass);
    }
}

/** A portal-frame metal shed: low gable, roof lights and a ridge vent. */
const portal: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.industry.metal, rand());
  if (!convex(b.ring)) return shed(mb, b, b.ring, b.height, wall, style, glass);
  const roof = roofColour(b, style);
  const top = roofed(mb, b.ring, 0, roofPlanes('gable', box, b.height, 10), wall, roof, 0.3);
  roofLights(mb, box, top, 1.2, glass);
  mb.box(
    box.c,
    top(box.c) - 0.3,
    box.L * 1.4,
    0.8,
    1.2,
    turn(box.u),
    roof.clone().multiplyScalar(0.85),
  );
  finish(mb, b, b.ring, b.height, style, glass);
};

/** A raised glazed monitor down the middle of a flat roof. */
const monitor: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.industry.walls, rand());
  shed(mb, b, b.ring, b.height, wall, style, glass);
  if (box.W < 5) return;
  const across = (p: Pt) => (p[0] - box.c[0]) * box.v[0] + (p[1] - box.c[1]) * box.v[1];
  const strip = clip(
    clip(b.ring, (p) => box.W * 0.3 - across(p)),
    (p) => box.W * 0.3 + across(p),
  );
  if (strip.length < 3) return;
  const roof = roofColour(b, style);
  flatRoofed(mb, strip, b.height, b.height + 2.2, glass, roof, 0.2, roof);
};

/** An arched hut: a tall curved roof springing from low walls. */
const arch: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.industry.metal, rand());
  if (b.ring.length !== 4 || !convex(b.ring))
    return shed(mb, b, b.ring, b.height, wall, style, glass);
  roofed(mb, b.ring, 0, roofPlanes('arch', box, b.height), wall, roofColour(b, style), 0.2);
  finish(mb, b, b.ring, b.height, style, glass);
};

/** A row of small business units: a coloured roller door and a sign each. */
const units: Part = (mb, b, _box, style, rand, glass) => {
  const s = style.industry;
  const wall = pick(s.walls, rand());
  const cap = wall.clone().multiplyScalar(0.88);
  flatRoofed(mb, b.ring, 0, b.height, wall, roofColour(b, style), 0.5, cap);
  const first = Math.floor(rand() * s.doors.length);
  const doors = s.doors.map((_, k) => new THREE.Color(s.doors[(first + k) % s.doors.length]));
  finish(mb, b, b.ring, b.height, style, glass, {}, doors);
  const e = edge(b.ring, b.front);
  const count = Math.floor((e.L - 4) / 7);
  const top = Math.min(4.6, b.height - 1.2);
  for (let k = 0; k < count; k++) {
    if (k > 0) mb.box(along(e, 1.5 + 7 * k, 0.1), 0, 0.4, b.height, 0.2, turn(e.u), cap);
    if (top + 1.2 <= b.height)
      mb.wall(
        along(e, 3.2 + 7 * k, PROUD),
        along(e, 6.8 + 7 * k, PROUD),
        top + 0.4,
        top + 1.1,
        doors[k % doors.length],
        e.n,
      );
  }
};

/** An old brick workshop under a slate gable, with tall framed windows and roof lights. */
const brick: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.industry.brick, rand());
  if (!convex(b.ring)) return shed(mb, b, b.ring, b.height, wall, style, glass);
  const pitch = Math.min(24, (Math.atan(5 / box.W) * 180) / Math.PI);
  const roof = new THREE.Color(style.resLow.slate);
  const top = roofed(mb, b.ring, 0, roofPlanes('gable', box, b.height, pitch), wall, roof, 0.3);
  roofLights(mb, box, top, 1.6, glass);
  const rows = { sill: 1.2, h: Math.max(1, b.height - 2.8), w: 1.4, gap: 2.2 };
  finish(mb, b, b.ring, b.height, style, glass, rows);
};

/** A feed or grain store: a shed at the front and a row of silos behind it. */
const silos: Part = (mb, b, box, style, rand, glass) => {
  const s = style.industry;
  const wall = pick(s.walls, rand());
  const e = edge(b.ring, b.front);
  const depth = Math.max(...b.ring.map((p) => depthBehind(e, p)));
  const keep = depth * 0.62;
  const r = Math.min((depth - keep) / 2 - 0.4, 3.2);
  if (r < 1.6 || !convex(b.ring)) return monitor(mb, b, box, style, rand, glass);
  const body = frontPart(b, keep);
  shed(mb, b, body, b.height, wall, style, glass);
  const n = Math.max(1, Math.min(3, Math.floor((e.L - 2) / (2 * r + 1))));
  const metal = pick(s.silo, rand());
  const top = b.height + 3 + rand() * 3;
  for (let k = 0; k < n; k++) {
    const c = along(e, e.L / 2 + (k - (n - 1) / 2) * (2 * r + 1), -(keep + (depth - keep) / 2));
    if (roomFor(b.ring, c, r))
      silo(mb, c, 0, r, top, r * 0.5, metal, metal.clone().multiplyScalar(0.9));
  }
};

const brickWorks = works('brick', brick);

export const WORKSHOPS: Works[] = [
  works('portal', portal),
  works('monitor', monitor),
  works('arch', arch),
  works('units', units),
  brickWorks,
  works('silos', silos),
];

/** Village workshops (ADR 0056): the classic shed and brick works, no modern units. */
export const VILLAGE_WORKSHOPS: Works[] = [classic, classic, brickWorks];
