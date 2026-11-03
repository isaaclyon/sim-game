// Display-only outdoor spaces; the standing building chooses the use, never the zone grid.
import * as THREE from 'three';
import { type Building, seeded } from '../building-kit/kit.ts';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { along, clip, convex, edge, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { boundary } from './boundary.ts';
import { commercialGeometry } from './commercial.ts';
import { industrialGeometry } from './industrial.ts';
import { fits, rectangle } from './placement.ts';
import { tree } from './props.ts';
import { residentialGeometry } from './residential.ts';

const lawn = [0x8fa66c, 0x98ad75, 0x849c64, 0x9aab79];
const paving = new THREE.Color(0xc8bda8);
const concrete = new THREE.Color(0xaaa99e);
const soil = new THREE.Color(0x76644c);
const hedge = new THREE.Color(0x56794d);
const timber = new THREE.Color(0xa58a65);

/** One site's meshes, also used by the tiled layer to batch many sites into one draw. */
export function lotGeometry(
  mb: MeshBuilder,
  lot: Pt[],
  b: Building,
  roads: Clearance,
  surface = mb,
) {
  if (b.zone >= 5) return industrialGeometry(mb, surface, lot, b, roads);
  if (b.zone >= 3) return commercialGeometry(mb, surface, lot, b, roads);
  const rand = seeded(b.ring);
  // Flat surfaces share the terrain plane without depth writes. Zones and roads draw later.
  surface.poly(lot, 0, new THREE.Color(lawn[Math.floor(rand() * lawn.length)]));
  if (b.zone === 1) return residentialGeometry(mb, surface, lot, b, roads);

  // Flats: a shared entrance, hedged grounds, planters and a rear garden.
  const front = edge(b.ring, b.front);
  const occupied: Pt[][] = [b.ring];
  // Reserve the entrance even on concave sites, where we omit the clipped paving.
  let path = rectangle(front, front.L / 2, 30, Math.min(5.5, front.L), 60);
  occupied.push(path);
  // Clipping is safe for convex lots; unusual concave sites keep their ground treatment.
  if (convex(lot)) {
    for (let i = 0; i < lot.length; i++) {
      const e = edge(lot, i);
      path = clip(path, (p) => -(p[0] - e.a[0]) * e.n[0] - (p[1] - e.a[1]) * e.n[1]);
    }
    if (path.length >= 3) surface.poly(path, 0, paving);
  }
  // Enclosures take priority over trees, beds and planters; those must fit inside the yard.
  occupied.push(...boundary(mb, lot, occupied, roads, 'hedge'));

  const place = (shape: Pt[], draw: () => void) => {
    if (!fits(shape, lot, occupied, roads)) return false;
    draw();
    occupied.push(shape);
    return true;
  };

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

  const back = edge(b.ring, (b.front + Math.floor(b.ring.length / 2)) % b.ring.length);
  for (const fraction of [0.25, 0.75]) {
    const s = back.L * fraction;
    place(rectangle(back, s, 5, 3.6, 3.6), () => tree(mb, along(back, s, 5), rand));
    const bed = rectangle(back, s, 2, 2.6, 1.2);
    place(bed, () => {
      mb.prism(bed, 0, 0.16, timber, true);
      mb.poly(rectangle(back, s, 2, 2.4, 1), 0.18, soil);
      for (const delta of [-0.8, 0, 0.8])
        mb.cylinder(along(back, s + delta, 2), 0.18, 0.3, 0.45, 5, hedge, 0.18);
    });
  }
}
