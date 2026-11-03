// Display-only outdoor spaces; the standing building chooses the use, never the zone grid.
import * as THREE from 'three';
import { type Building, seeded } from '../building-kit/kit.ts';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { along, clip, convex, edge, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { boundary } from './boundary.ts';
import { industrialGeometry } from './industrial.ts';
import { fits, rectangle } from './placement.ts';
import { residentialGeometry } from './residential.ts';

const lawn = [0x8fa66c, 0x98ad75, 0x849c64, 0x9aab79];
const paving = new THREE.Color(0xc8bda8);
const concrete = new THREE.Color(0xaaa99e);
const asphalt = new THREE.Color(0x858b87);
const soil = new THREE.Color(0x76644c);
const hedge = new THREE.Color(0x56794d);
const leaves = [0x6b9054, 0x7e9b5e, 0x527c51];
const timber = new THREE.Color(0xa58a65);

/** One site's meshes, also used by the tiled layer to batch many sites into one draw. */
export function lotGeometry(
  mb: MeshBuilder,
  lot: Pt[],
  b: Building,
  roads: Clearance,
  surface = mb,
) {
  if (b.zone >= 5) {
    industrialGeometry(mb, surface, lot, b, roads);
    return;
  }
  const rand = seeded(b.ring);
  const home = b.zone <= 2;
  const ground =
    home || b.zone === 3 ? new THREE.Color(lawn[Math.floor(rand() * lawn.length)]) : paving;
  // Flat surfaces share the terrain plane without depth writes. Zones and roads draw later.
  surface.poly(lot, 0, ground);
  if (b.zone === 1) {
    residentialGeometry(mb, surface, lot, b, roads);
    return;
  }
  const front = edge(b.ring, b.front);
  const occupied: Pt[][] = [b.ring];

  // A clear, broad entrance also accommodates the house kit's slightly off-centre door.
  // Reserve the entrance even on concave sites, where we omit the clipped paving.
  let path = rectangle(front, front.L / 2, 30, home ? Math.min(5.5, front.L) : front.L * 0.7, 60);
  occupied.push(path);
  // Clipping is safe for convex lots; unusual concave sites keep their ground treatment.
  if (convex(lot)) {
    for (let i = 0; i < lot.length; i++) {
      const e = edge(lot, i);
      path = clip(path, (p) => -(p[0] - e.a[0]) * e.n[0] - (p[1] - e.a[1]) * e.n[1]);
    }
    if (path.length >= 3) {
      surface.poly(path, 0, paving);
    }
  }

  // Enclosures take priority over trees, beds and planters; those must fit inside the yard.
  if (home) occupied.push(...boundary(mb, lot, occupied, roads, 'hedge'));

  const place = (shape: Pt[], draw: () => void) => {
    if (!fits(shape, lot, occupied, roads)) return false;
    draw();
    occupied.push(shape);
    return true;
  };

  // Slim planting boxes fit the small setback of shops with otherwise shared walls.
  if (!home) {
    for (const fraction of [0.1, 0.9]) {
      const shape = rectangle(front, front.L * fraction, 0.4, 1.1, 0.35);
      place(shape, () => {
        mb.prism(shape, 0, 0.35, concrete, true);
        mb.prism(rectangle(front, front.L * fraction, 0.4, 0.95, 0.25), 0.35, 0.65, hedge, true);
      });
    }
  }

  // Planters beside entrances and gardens behind homes.
  // Dense buildings often fill their lots: try shallow planters, never shrink the building.
  for (let i = 0; i < b.ring.length; i++) {
    const e = edge(b.ring, i);
    const count = Math.min(4, Math.floor(e.L / 5));
    for (let k = 0; k < count; k++) {
      const s = e.L * ((k + 0.5) / count);
      if (i === b.front && s > e.L * 0.2 && s < e.L * 0.8) continue;
      const shape = rectangle(e, s, 0.7, 1.5, 0.55);
      place(shape, () => {
        mb.prism(shape, 0.03, 0.32, concrete, true);
        mb.prism(rectangle(e, s, 0.7, 1.34, 0.43), 0.32, 0.7, hedge, true);
      });
    }
  }

  if (!home) {
    const side = edge(b.ring, (b.front + 1) % b.ring.length);
    // A single marked bay, not a sea of parking. No parked vehicle or capacity is implied.
    const bay = rectangle(side, side.L * 0.7, 4, 2.8, 5);
    place(bay, () => {
      surface.poly(bay, 0, asphalt);
      const paint = new THREE.Color(0xd4cdb7);
      for (const s of [-1.25, 1.25])
        surface.poly(rectangle(side, side.L * 0.7 + s, 4, 0.12, 4.8), 0, paint);
      surface.poly(rectangle(side, side.L * 0.7, 1.65, 2.5, 0.12), 0, paint);
    });
    const seat = rectangle(side, side.L * 0.25, 1.8, 2, 0.6);
    place(seat, () => {
      for (const s of [-0.7, 0.7])
        mb.prism(rectangle(side, side.L * 0.25 + s, 1.8, 0.15, 0.5), 0, 0.45, concrete, true);
      mb.prism(seat, 0.45, 0.58, timber, true);
      mb.prism(rectangle(side, side.L * 0.25, 1.55, 2, 0.12), 0.58, 1.05, timber, true);
    });
  }

  if (home || b.zone === 3) {
    // Small shops share the planted rear space, but not the home's vegetable beds.
    const back = edge(b.ring, (b.front + Math.floor(b.ring.length / 2)) % b.ring.length);
    for (const fraction of home ? [0.25, 0.75] : [0.5]) {
      const s = back.L * fraction;
      const shape = rectangle(back, s, 5, 3.6, 3.6);
      const c = along(back, s, 5);
      place(shape, () => {
        mb.cylinder(c, 0, 0.17, 2.2, 5, timber);
        mb.cylinder(
          c,
          1.5,
          1.65,
          2.5 + rand(),
          7,
          new THREE.Color(leaves[Math.floor(rand() * leaves.length)]),
          0.25,
        );
      });
      if (!home) continue;
      const bed = rectangle(back, s, 2, 2.6, 1.2);
      place(bed, () => {
        mb.prism(bed, 0, 0.16, timber, true);
        mb.poly(rectangle(back, s, 2, 2.4, 1), 0.18, soil);
        for (const delta of [-0.8, 0, 0.8])
          mb.cylinder(along(back, s + delta, 2), 0.18, 0.3, 0.45, 5, hedge, 0.18);
      });
    }
  }
}
