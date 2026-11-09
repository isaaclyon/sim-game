// Small shops: one trading floor with a shopfront, an entrance 30% along the front (where the
// lot's walkway meets it), a fascia and, on two-storey shops, a row of upper windows. The
// designs differ above and around that shared street level.
import * as THREE from 'three';
import { door, shopfront, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import {
  type Archetype,
  type Building,
  frontBox,
  type Part,
  pick,
  roomFor,
  trimFor,
  turn,
  turned,
} from './parts.ts';
import { along, convex, edge } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';
import type { Style } from './style.ts';
import { band, plinth } from './trim.ts';

interface Street {
  /** Awning colour, or null for none. */
  awning: THREE.Color | null;
  stripes?: THREE.Color;
  /** Fascia colour; the awning's by default. */
  fascia?: THREE.Color;
}

/** The street level every small shop shares, and its upper windows below `body`. */
function trading(
  mb: MeshBuilder,
  b: Building,
  style: Style,
  wall: THREE.Color,
  glass: THREE.Color,
  street: Street,
) {
  const base = new THREE.Color(style.plinth);
  plinth(mb, b.ring, 0.35, base);
  const e = edge(b.ring, b.front);
  shopfront(mb, b.ring, b.front, street.awning, style, e.L * 0.3, street.stripes ?? null);
  const frame = trimFor(wall, style);
  door(mb, b.ring, b.front, e.L * 0.3, new THREE.Color(style.door), frame, base);
  // A shop-sized fascia above its own awning, not a continuous block-wide band.
  const fascia = street.fascia ?? street.awning ?? frame;
  mb.wall(along(e, 0.6, 0.18), along(e, e.L - 0.6, 0.18), 3.5, 3.95, fascia, e.n);
  const rows = { storey: 4, first: 4, floors: Math.floor(b.height / 4) - 1, h: 1.4, w: 1.3 };
  windows(mb, b.ring, () => b.height, rows, glass, frame);
}

interface Shell {
  wall: THREE.Color;
  roof: THREE.Color;
  cap: THREE.Color;
}

function shell(style: Style, rand: () => number): Shell {
  const wall = pick(style.comLow.walls, rand());
  return { wall, roof: new THREE.Color(style.comLow.roof), cap: wall.clone().multiplyScalar(0.9) };
}

const flatTop = (mb: MeshBuilder, b: Building, s: Shell, parapet = 0.35) =>
  flatRoofed(mb, b.ring, 0, b.height, s.wall, s.roof, parapet, s.cap);

const awning = (style: Style, rand: () => number) => pick(style.awnings, rand());

const flat: Part = (mb, b, _box, style, rand, glass) => {
  const s = shell(style, rand);
  flatTop(mb, b, s);
  trading(mb, b, style, s.wall, glass, { awning: awning(style, rand) });
};

/** A shallow hipped roof; concave footprints keep a flat one. */
const hip: Part = (mb, b, box, style, rand, glass) => {
  const s = shell(style, rand);
  if (convex(b.ring))
    roofed(mb, b.ring, 0, roofPlanes('hip', box, b.height, 18), s.wall, s.roof, 0.25);
  else flatTop(mb, b, s);
  trading(mb, b, style, s.wall, glass, { awning: awning(style, rand) });
};

/** A tall false front hides the roof from the street, with a raised centre panel. */
const falseFront: Part = (mb, b, _box, style, rand, glass) => {
  const s = shell(style, rand);
  flatTop(mb, b, s);
  const e = edge(b.ring, b.front);
  const rise = 1.3 + rand() * 0.8;
  const face = s.wall.clone().multiplyScalar(0.96);
  mb.box(along(e, e.L / 2, -0.15), b.height, e.L, rise, 0.3, turn(e.u), face);
  mb.box(along(e, e.L / 2, -0.15), b.height + rise, e.L * 0.4, 0.5, 0.3, turn(e.u), s.cap);
  trading(mb, b, style, s.wall, glass, { awning: awning(style, rand) });
};

/** A roadside diner: a shallow barrel roof over a chrome band. */
const diner: Part = (mb, b, box, style, rand, glass) => {
  const s = shell(style, rand);
  if (!convex(b.ring)) return flat(mb, b, box, style, rand, glass);
  roofed(mb, b.ring, 0, roofPlanes('barrel', box, b.height), s.wall, s.roof, 0.3);
  band(mb, b.ring, b.height - 0.35, b.height - 0.1, 0.08, new THREE.Color(style.comLow.chrome));
  trading(mb, b, style, s.wall, glass, { awning: awning(style, rand) });
};

/** A gable end to the street, with a sign board in the gable when it has room. */
const gableFront: Part = (mb, b, box, style, rand, glass) => {
  const s = shell(style, rand);
  if (!convex(b.ring)) return flat(mb, b, box, style, rand, glass);
  const rb = turned(frontBox(b, box));
  // Keep the ridge low enough to read as a shop, not a house.
  const slope = Math.min(Math.tan((22 * Math.PI) / 180), 2.4 / rb.W);
  const pitch = (Math.atan(slope) * 180) / Math.PI;
  const roof = pick(style.resLow.roofs, rand());
  roofed(mb, b.ring, 0, roofPlanes('gable', rb, b.height, pitch), s.wall, roof, 0.3);
  const colour = awning(style, rand);
  const e = edge(b.ring, b.front);
  if (slope * (rb.W - 1.4) > 1)
    mb.wall(
      along(e, e.L / 2 - 1.2, 0.1),
      along(e, e.L / 2 + 1.2, 0.1),
      b.height + 0.15,
      b.height + 0.85,
      colour,
      e.n,
    );
  trading(mb, b, style, s.wall, glass, { awning: colour });
};

/** A deep flat canopy on posts across the whole front, instead of an awning. */
const canopy: Part = (mb, b, _box, style, rand, glass) => {
  const s = shell(style, rand);
  flatTop(mb, b, s, 0.5);
  const e = edge(b.ring, b.front);
  const rot = turn(e.u);
  const chrome = new THREE.Color(style.comLow.chrome);
  mb.box(
    along(e, e.L / 2, 0.95),
    3.4,
    e.L - 0.2,
    0.25,
    1.9,
    rot,
    new THREE.Color(style.comLow.canopy),
  );
  for (const at of [0.4, e.L - 0.4]) mb.box(along(e, at, 1.7), 0, 0.18, 3.4, 0.18, rot, chrome);
  trading(mb, b, style, s.wall, glass, { awning: null, fascia: awning(style, rand) });
};

/** A lit sign board on posts over the front of a flat roof. */
const sign: Part = (mb, b, _box, style, rand, glass) => {
  const s = shell(style, rand);
  flatTop(mb, b, s);
  const e = edge(b.ring, b.front);
  const rot = turn(e.u);
  const w = Math.min(e.L * 0.7, 8);
  const steel = new THREE.Color(style.industry.steel);
  for (const side of [-1, 1])
    mb.box(along(e, e.L / 2 + side * w * 0.35, -1), b.height, 0.15, 0.75, 0.15, rot, steel);
  mb.glow.set(0x8a6d48);
  mb.box(along(e, e.L / 2, -1), b.height + 0.7, w, 1.5, 0.2, rot, awning(style, rand));
  mb.glow.set(0x000000);
  trading(mb, b, style, s.wall, glass, { awning: awning(style, rand) });
};

/** A café: a striped awning and parasols on a roof terrace behind a tall parapet. */
const cafe: Part = (mb, b, box, style, rand, glass) => {
  const s = shell(style, rand);
  flatTop(mb, b, s, 0.8);
  const colour = awning(style, rand);
  const stripes = new THREE.Color(style.comLow.canopy);
  const pole = new THREE.Color(style.industry.steel);
  for (const f of [-0.35, 0.35]) {
    const c: [number, number] = [box.c[0] + box.u[0] * box.L * f, box.c[1] + box.u[1] * box.L * f];
    if (!roomFor(b.ring, c, 1.6)) continue;
    mb.cylinder(c, b.height, 0.06, 2.2, 4, pole);
    mb.cylinder(c, b.height + 1.9, 1.3, 0.5, 8, f < 0 ? colour : stripes, 0);
  }
  trading(mb, b, style, s.wall, glass, { awning: colour, stripes });
};

const hipped: Archetype = { name: 'hip', draw: hip };
const falseFronted: Archetype = { name: 'false-front', draw: falseFront };
const gableFronted: Archetype = { name: 'gable-front', draw: gableFront };

export const SHOPS: Archetype[] = [
  { name: 'flat', draw: flat },
  hipped,
  falseFronted,
  { name: 'diner', draw: diner },
  gableFronted,
  { name: 'canopy', draw: canopy },
  { name: 'sign', draw: sign },
  { name: 'cafe', draw: cafe },
];

/** Village shops (ADR 0059): pitched roofs and false fronts, no diners or chrome canopies. */
export const VILLAGE_SHOPS: Archetype[] = [hipped, falseFronted, gableFronted, gableFronted];
