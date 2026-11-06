// Stores: dense commercial blocks on the whole lot, glazed across their street level. Upper
// floors are 4 m. Designs that step back or grow a crown need a convex footprint and fall
// back to the plain office block otherwise.
import * as THREE from 'three';
import { pane, rooftop, shopfront, windows } from './facade.ts';
import type { MeshBuilder } from './mesh.ts';
import { type Archetype, type Building, clipBox, type Part, pick, trimFor, turn } from './parts.ts';
import { along, at, convex, edge, obb, type Pt } from './polygon.ts';
import { flatRoofed, roofed, roofPlanes } from './roof.ts';
import type { Style } from './style.ts';
import { band, PROUD, plinth } from './trim.ts';

/** Upper-floor window rows on floors starting at `first`, below a roof at `top`. */
const upper = (first: number, top: number) => ({
  storey: 4,
  first,
  floors: Math.floor((top - first) / 4),
});

/** Plinth and street-level glazing, with an awning if one is given. */
function street(mb: MeshBuilder, b: Building, style: Style, awning: THREE.Color | null) {
  plinth(mb, b.ring, 0.35, new THREE.Color(style.plinth));
  shopfront(mb, b.ring, b.front, awning, style, null);
}

const office: Part = (mb, b, box, style, rand, glass) => {
  const s = style.comHigh;
  const wall = pick(s.walls, rand());
  const roof = new THREE.Color(s.roof);
  const cap = wall.clone().multiplyScalar(0.9);
  // Taller blocks on convex lots step back to a smaller top floor.
  const setback = b.height > 20 && rand() < 0.6 && convex(b.ring);
  const body = setback ? b.height - 4 : b.height;
  flatRoofed(mb, b.ring, 0, body, wall, roof, 0.6, cap);
  if (setback) {
    const inner = clipBox(b.ring, box, 2.2);
    if (inner.length >= 3) flatRoofed(mb, inner, body, b.height, wall, roof, 0.5, cap);
  }
  street(mb, b, style, rand() < 0.5 ? pick(style.awnings, rand()) : null);
  // Glass office blocks keep plain windows; their panes nearly touch.
  const rows = { ...upper(4, body), sill: 0.6, h: 2.2, w: 2.6, gap: 0.6 };
  windows(mb, b.ring, () => body, rows, glass, null);
  rooftop(mb, b.ring, box, b.height, 3, false, rand, new THREE.Color(style.rooftop));
};

/** A stone department store: pilasters between tall windows and a heavy cornice. */
const department: Part = (mb, b, _box, style, rand, glass) => {
  const wall = pick(style.comHigh.stone, rand());
  const cap = wall.clone().multiplyScalar(0.86);
  flatRoofed(mb, b.ring, 0, b.height, wall, new THREE.Color(style.comHigh.roof), 1, cap);
  band(mb, b.ring, b.height - 1.2, b.height - 0.8, 0.4, cap, true);
  street(mb, b, style, pick(style.awnings, rand()));
  const [w, gap] = [1.4, 1.6];
  const rows = { ...upper(4, b.height), sill: 0.8, h: 1.9, w, gap };
  windows(mb, b.ring, () => b.height, rows, glass, trimFor(wall, style));
  // Pilasters fall between the front's window columns, as `windows` spaces them.
  const e = edge(b.ring, b.front);
  const count = Math.max(1, Math.floor((e.L - 1) / (w + gap)));
  const start = (e.L - (count * w + (count - 1) * gap)) / 2;
  for (let k = 0; k <= count; k++) {
    const s = start - gap / 2 + k * (w + gap);
    if (s > 0.3 && s < e.L - 0.3)
      mb.box(along(e, s, 0.12), 4, 0.5, b.height - 5.2, 0.24, turn(e.u), cap);
  }
};

/** Modernist ribbon windows: continuous glass bands between pale spandrels. */
const ribbon: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.comHigh.walls, rand()).lerp(new THREE.Color(0xffffff), 0.5);
  const cap = wall.clone().multiplyScalar(0.9);
  flatRoofed(mb, b.ring, 0, b.height, wall, new THREE.Color(style.comHigh.roof), 0.4, cap);
  street(mb, b, style, null);
  const { floors } = upper(4, b.height);
  for (let i = 0; i < b.ring.length; i++) {
    const e = edge(b.ring, i);
    if (e.L < 3) continue;
    const n = Math.max(1, Math.round((e.L - 1) / 3));
    const w = (e.L - 1) / n;
    for (let f = 0; f < floors; f++)
      for (let k = 0; k < n; k++) {
        const [p, q] = [
          along(e, 0.5 + k * w + 0.06, PROUD),
          along(e, 0.5 + (k + 1) * w - 0.06, PROUD),
        ];
        pane(mb, p, q, 4 + f * 4 + 0.9, 4 + f * 4 + 3.1, glass, e.n, f);
      }
  }
  rooftop(mb, b.ring, box, b.height, 2, false, rand, new THREE.Color(style.rooftop));
};

/** A glass tower set back on a two-storey stone podium. */
const podium: Part = (mb, b, box, style, rand, glass) => {
  const inner = convex(b.ring) && box.W > 7 && b.height >= 16 ? clipBox(b.ring, box, 3) : [];
  if (inner.length < 3) return office(mb, b, box, style, rand, glass);
  const s = style.comHigh;
  const stone = pick(s.stone, rand());
  const roof = new THREE.Color(s.roof);
  flatRoofed(mb, b.ring, 0, 8, stone, roof, 0.8, stone.clone().multiplyScalar(0.88));
  street(mb, b, style, rand() < 0.5 ? pick(style.awnings, rand()) : null);
  const low = { ...upper(4, 8), sill: 0.8, h: 2, w: 1.6, gap: 1.4 };
  windows(mb, b.ring, () => 8, low, glass, trimFor(stone, style));
  const wall = pick(s.walls, rand());
  flatRoofed(mb, inner, 8, b.height, wall, roof, 0.6, wall.clone().multiplyScalar(0.9));
  const tower = { ...upper(8, b.height), sill: 0.5, h: 2.6, w: 2.8, gap: 0.4 };
  windows(mb, inner, () => b.height, tower, glass, null);
  rooftop(mb, inner, obb(inner), b.height, 2, false, rand, new THREE.Color(style.rooftop));
};

/** A steep copper pyramid roof and spire over the block. */
const crown: Part = (mb, b, box, style, rand, glass) => {
  office(mb, b, box, style, rand, glass);
  // Clear of the setback `office` may step back to.
  const inner = convex(b.ring) ? clipBox(b.ring, box, 2.6) : [];
  if (inner.length < 3) return;
  const ib = obb(inner);
  const pitch = Math.min(50, (Math.atan(7 / ib.W) * 180) / Math.PI);
  const copper = new THREE.Color(style.comHigh.copper);
  const top = roofed(mb, inner, b.height, roofPlanes('hip', ib, b.height, pitch), copper, copper);
  mb.cylinder(ib.c, top(ib.c) - 0.5, 0.2, 3.5, 6, copper, 0);
};

/** A wedding cake: two setbacks, each one storey up. */
const stepped: Part = (mb, b, box, style, rand, glass) => {
  const tiers = convex(b.ring) && box.W > 8 && b.height >= 16;
  const t1 = tiers ? clipBox(b.ring, box, 2.5) : [];
  const t2 = tiers ? clipBox(b.ring, box, 5) : [];
  if (t1.length < 3 || t2.length < 3) return office(mb, b, box, style, rand, glass);
  const s = style.comHigh;
  const wall = pick(s.stone, rand());
  const cap = wall.clone().multiplyScalar(0.88);
  const roof = new THREE.Color(s.roof);
  const frame = trimFor(wall, style);
  const rows = { sill: 0.6, h: 2.2, w: 2, gap: 1 };
  const levels: [Pt[], number, number][] = [
    [b.ring, 0, b.height - 8],
    [t1, b.height - 8, b.height - 4],
    [t2, b.height - 4, b.height],
  ];
  for (const [ring, base, top] of levels) {
    flatRoofed(mb, ring, base, top, wall, roof, 0.6, cap);
    windows(mb, ring, () => top, { ...upper(Math.max(4, base), top), ...rows }, glass, frame);
  }
  street(mb, b, style, pick(style.awnings, rand()));
  rooftop(mb, t2, obb(t2), b.height, 1, false, rand, new THREE.Color(style.rooftop));
};

/** A lit blade sign down one corner of the front and a billboard on the roof. */
const signs: Part = (mb, b, box, style, rand, glass) => {
  const wall = pick(style.resHigh.brick, rand());
  const cap = wall.clone().multiplyScalar(0.85);
  flatRoofed(mb, b.ring, 0, b.height, wall, new THREE.Color(style.comHigh.roof), 0.6, cap);
  street(mb, b, style, pick(style.awnings, rand()));
  const rows = { ...upper(4, b.height), sill: 0.8, h: 2, w: 1.6, gap: 1.2 };
  windows(mb, b.ring, () => b.height, rows, glass, trimFor(wall, style));
  const e = edge(b.ring, b.front);
  const sign = pick(style.awnings, rand());
  mb.glow.set(0x8a6d48);
  mb.box(along(e, 1.2, 0.75), 5, 0.35, b.height - 7, 1.5, turn(e.u), sign);
  const c = at(box, 0, 0);
  const w = Math.min(box.L * 1.2, 12);
  mb.box(c, b.height + 1.5, w, 3, 0.3, turn(box.u), pick(style.awnings, rand()));
  mb.glow.set(0x000000);
  const steel = new THREE.Color(style.industry.steel);
  for (const side of [-1, 1])
    mb.box(at(box, side * w * 0.35, 0), b.height, 0.2, 1.5, 0.2, turn(box.u), steel);
};

export const STORES: Archetype[] = [
  { name: 'office', draw: office },
  { name: 'department', draw: department },
  { name: 'ribbon', draw: ribbon },
  { name: 'podium', draw: podium },
  { name: 'crown', draw: crown },
  { name: 'stepped', draw: stepped },
  { name: 'signs', draw: signs },
];
