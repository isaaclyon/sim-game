// Houses: low-density homes. Each design is a roof over one or two storeys with a front door
// where the garden path meets it. Concave footprints keep a flat roof whatever the design.
import * as THREE from 'three';
import { door, type WindowRows, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import {
  type Archetype,
  type Building,
  frontBox,
  type Part,
  pick,
  seeded,
  trimFor,
  turn,
  turned,
} from './parts.ts';
import { along, at, type Box, convex, edge } from './polygon.ts';
import { flatRoofed, type Roof, type RoofForm, roofed, roofPlanes } from './roof.ts';
import type { Style } from './style.ts';
import { PLINTH, plinth } from './trim.ts';

/** Shared by the house facade and its garden path; independent of other visual choices. */
export function houseDoorAt(b: Building): number {
  return edge(b.ring, b.front).L * (0.35 + seeded(b.ring)() * 0.3);
}

interface Look {
  form: RoofForm;
  /** Roof pitch range in degrees; the style's for plain gables and hips. */
  pitch?: [number, number];
  /** `long`: ridge along the long side; `front`: along the street, for one-sided roofs;
   * `across`: from the street back, so the gable end faces the road. */
  ridge?: 'long' | 'front' | 'across';
  walls?: number[];
  /** A slate roof instead of the style's tiles. */
  slate?: boolean;
  overhang?: number;
  /** Share of houses with a chimney. */
  chimney?: number;
  window?: Partial<WindowRows>;
}

interface Home {
  top: Roof;
  roof: THREE.Color;
  frame: THREE.Color;
  doorAt: number;
}

function home(
  mb: MeshBuilder,
  b: Building,
  box: Box,
  style: Style,
  rand: () => number,
  glass: THREE.Color,
  look: Look,
): Home {
  const s = style.resLow;
  const wall = pick(look.walls ?? s.walls, rand());
  const roof = look.slate ? new THREE.Color(s.slate) : pick(s.roofs, rand());
  const [lo, hi] = look.pitch ?? s.pitch;
  const pitch = lo + rand() * (hi - lo);
  const pitched = convex(b.ring);
  const base = new THREE.Color(style.plinth);
  plinth(mb, b.ring, PLINTH, base);
  const frame = trimFor(wall, style);
  const ridge = look.ridge ?? 'long';
  const rb =
    ridge === 'long' ? box : ridge === 'front' ? frontBox(b, box) : turned(frontBox(b, box));
  const top = pitched
    ? roofed(
        mb,
        b.ring,
        0,
        roofPlanes(look.form, rb, b.height, pitch),
        wall,
        roof,
        look.overhang ?? 0.45,
      )
    : flatRoofed(mb, b.ring, 0, b.height, wall, roof, 0.4, wall);
  const doorAt = houseDoorAt(b);
  const floors = Math.max(1, Math.round(b.height / 3));
  const rows = { storey: 3, floors, h: 1.25, ...look.window, door: { edge: b.front, at: doorAt } };
  windows(mb, b.ring, top, rows, glass, frame);
  door(mb, b.ring, b.front, doorAt, new THREE.Color(style.door), frame, base);
  if (pitched && rand() < (look.chimney ?? 0.65)) {
    const c = at(rb, rb.L * (rand() - 0.5) * 0.8, 0);
    mb.box(c, top(c) - 1.2, 0.8, 2.3, 0.8, turn(rb.u), new THREE.Color(style.chimney));
  }
  return { top, roof, frame, doorAt };
}

const plain =
  (look: Look): Part =>
  (mb, b, box, style, rand, glass) => {
    home(mb, b, box, style, rand, glass, look);
  };

/** A front-gabled cottage with a covered porch along its front. */
const porch: Part = (mb, b, box, style, rand, glass) => {
  const look: Look = { form: 'gable', pitch: [38, 45], ridge: 'across', chimney: 0.4 };
  const h = home(mb, b, box, style, rand, glass, look);
  const e = edge(b.ring, b.front);
  if (e.L < 6 || !convex(b.ring)) return;
  const [s0, s1] = [Math.max(0.3, h.doorAt - 3), Math.min(e.L - 0.3, h.doorAt + 3)];
  const y = 2.7;
  const [a, q] = [along(e, s0), along(e, s1)];
  const [c, d] = [along(e, s1, 1.9), along(e, s0, 1.9)];
  mb.quad([a[0], y + 0.4, a[1]], [q[0], y + 0.4, q[1]], [c[0], y, c[1]], [d[0], y, d[1]], h.roof, [
    e.n[0],
    1,
    e.n[1],
  ]);
  mb.wall(d, c, y - 0.25, y, h.frame, e.n);
  const rot = turn(e.u);
  for (const s of [s0 + 0.15, s1 - 0.15]) mb.box(along(e, s, 1.75), 0, 0.18, y, 0.18, rot, h.frame);
  mb.box(along(e, (s0 + s1) / 2, 0.95), 0, s1 - s0, 0.22, 1.9, rot, new THREE.Color(style.plinth));
};

/** Dark timber or white render under a shallow single-pitch roof, with wide windows. */
const modern: Part = (mb, b, box, style, rand, glass) => {
  const look: Look = {
    form: 'shed',
    pitch: [6, 10],
    ridge: 'front',
    walls: style.resLow.modern,
    slate: true,
    overhang: 0.6,
    chimney: 0,
    window: { w: 2.2, gap: 1, h: 1.6, sill: 0.6 },
  };
  const h = home(mb, b, box, style, rand, glass, look);
  const e = edge(b.ring, b.front);
  if (e.L >= 4) mb.box(along(e, h.doorAt, 0.6), 2.5, 2.2, 0.15, 1.2, turn(e.u), h.roof);
};

const gable: Archetype = { name: 'gable', draw: plain({ form: 'gable' }) };
const saltbox: Archetype = {
  name: 'saltbox',
  draw: plain({ form: 'saltbox', pitch: [40, 46], ridge: 'front' }),
};
const jerkinhead: Archetype = {
  name: 'jerkinhead',
  draw: plain({ form: 'jerkinhead', pitch: [34, 42] }),
};
const porched: Archetype = { name: 'porch', draw: porch };

export const HOUSES: Archetype[] = [
  gable,
  { name: 'hip', draw: plain({ form: 'hip' }) },
  saltbox,
  {
    name: 'gambrel',
    draw: plain({ form: 'gambrel', pitch: [16, 22], overhang: 0.15, chimney: 0.3 }),
  },
  {
    name: 'mansard',
    draw: plain({ form: 'mansard', pitch: [10, 16], slate: true, overhang: 0.15, chimney: 0.8 }),
  },
  jerkinhead,
  porched,
  { name: 'modern', draw: modern },
];

/** Village homes (ADR 0059): steep roofs with deep eaves and a chimney, small windows. */
export const VILLAGE_HOUSES: Archetype[] = [
  {
    name: 'cottage',
    draw: plain({
      form: 'gable',
      pitch: [46, 52],
      overhang: 0.6,
      chimney: 0.95,
      window: { w: 0.9, h: 1 },
    }),
  },
  {
    name: 'thatched',
    draw: plain({ form: 'hip', pitch: [44, 50], overhang: 0.8, chimney: 0.8, window: { w: 0.9 } }),
  },
  gable,
  saltbox,
  jerkinhead,
  porched,
];
