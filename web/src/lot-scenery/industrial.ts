// Coherent, footprint-seeded factory yards. None of these spaces confer parking or capacity.
import * as THREE from 'three';
import { type Building, seeded } from '../building-kit/kit.ts';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { along, type Edge, edge, inside, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { type BoundaryStyle, boundary } from './boundary.ts';
import { access, fits, rectangle } from './placement.ts';

const grounds = [0x858b87, 0x9d9c94, 0x8e877b]; // Asphalt, concrete, gravel.
const asphalt = new THREE.Color(0x858b87);
const concrete = new THREE.Color(0xaaa99e);
const bayAsphalt = new THREE.Color(0x6f7470);
const paint = new THREE.Color(0xd4cdb7);
const safety = new THREE.Color(0xcfb35a);
const timber = new THREE.Color(0xa58a65);
const tank = new THREE.Color(0xc9ccc6);
const goods = [0x9caa9d, 0xb4996e, 0xc7c3b5];
const boxes = [0x9b4a3c, 0x3f6d8c, 0x6f7f4a, 0xb08a3e];
const skips = [0x4f6b4a, 0xc39a3a];
const pick = (list: number[], r: number) => new THREE.Color(list[Math.floor(r * list.length)]);

/** Loading doors match `loadingDoors` in the building kit: 4 m wide, every 7 m from 3 m. */
const doorCount = (e: Edge) => Math.max(0, Math.floor((e.L - 4) / 7));

export function industrialGeometry(
  mb: MeshBuilder,
  surface: MeshBuilder,
  lot: Pt[],
  b: Building,
  roads: Clearance,
) {
  // A different seed from the building kit's, so yard choices don't track wall colours.
  const rand = seeded(b.ring.slice(1));
  const groundKind = Math.floor(rand() * grounds.length);
  const enclosure: BoundaryStyle = (['metal', 'metal', 'wall', 'none'] as const)[
    Math.floor(rand() * 4)
  ];
  const yard = Math.floor(rand() * 4);
  const wideApron = rand() < 0.5;
  surface.poly(lot, 0, new THREE.Color(grounds[groundKind]));

  const front = edge(b.ring, b.front);
  const occupied = [b.ring];
  const doors = doorCount(front);
  // A truck court across every loading door, or one drive to the middle door.
  let apron: Pt[] | null = null;
  if (wideApron && doors >= 2) {
    const to = 3 + (doors - 1) * 7 + 6;
    apron = access(lot, front, (1 + to) / 2, to - 1, 0.08, occupied, roads);
    const court = apron;
    // Lines either side of each door; drawn only where they lie on the apron itself.
    for (let k = 0; court && k <= doors; k++) {
      const s = k < doors ? 2.5 + k * 7 : 7.5 + (doors - 1) * 7;
      const line = rectangle(front, s, 3.5, 0.12, 6);
      if (line.every((p) => inside(p, court))) surface.poly(line, 0, safety);
    }
  }
  if (!apron) {
    const s = doors ? 5 + Math.floor((doors - 1) / 2) * 7 : front.L / 2;
    apron = access(lot, front, s, Math.min(8, front.L * 0.7), 0.08, occupied, roads);
  }
  if (apron) surface.poly(apron, 0, groundKind === 1 ? asphalt : concrete);
  // Keep the gate open even where no apron fits, as on sites cut by a main road.
  occupied.push(apron ?? rectangle(front, front.L / 2, 30, Math.min(8, front.L * 0.7), 60));
  occupied.push(...boundary(mb, lot, occupied, roads, enclosure));

  const place = (shape: Pt[], draw: () => void) => {
    if (!fits(shape, lot, occupied, roads)) return false;
    draw();
    occupied.push(shape);
    return true;
  };
  // Side, back, other side: try evenly spaced spots on each edge until `max` items fit.
  const sides = Array.from(
    { length: b.ring.length - 1 },
    (_, k) => (b.front + 1 + k) % b.ring.length,
  );
  const scatter = (
    width: number,
    depth: number,
    max: number,
    draw: (e: Edge, s: number, out: number) => void,
  ) => {
    let placed = 0;
    for (const i of sides) {
      const e = edge(b.ring, i);
      const count = Math.floor((e.L - 1) / (width + 1.2));
      for (let k = 0; k < count && placed < max; k++) {
        const s = e.L * ((k + 0.5) / count);
        for (const out of [depth / 2 + 1.2, depth / 2 + 3]) {
          if (place(rectangle(e, s, out, width, depth), () => draw(e, s, out))) {
            placed++;
            break;
          }
        }
      }
    }
    return placed;
  };
  const bay = (e: Edge, s: number, out: number) => {
    surface.poly(rectangle(e, s, out, 2.8, 5), 0, bayAsphalt);
    for (const d of [-1.3, 1.3]) surface.poly(rectangle(e, s + d, out, 0.12, 4.8), 0, paint);
  };

  if (yard === 0) {
    // Pallets of goods, four to a stack footprint, at varied heights.
    scatter(2.6, 2.6, 6, (e, s, out) => {
      for (const ds of [-0.65, 0.65])
        for (const dout of [-0.65, 0.65]) {
          const base = rectangle(e, s + ds, out + dout, 1.2, 1.2);
          mb.prism(base, 0, 0.14, timber, true);
          const load = rectangle(e, s + ds, out + dout, 1.1, 1.1);
          mb.prism(load, 0.14, 0.7 + rand() * 0.8, pick(goods, rand()), true);
        }
    });
  } else if (yard === 1) {
    // Shipping containers along the yard edge, sometimes stacked two high.
    scatter(6.1, 2.5, 4, (e, s, out) => {
      mb.prism(rectangle(e, s, out, 6.06, 2.44), 0, 2.6, pick(boxes, rand()), true);
      if (rand() < 0.35)
        mb.prism(rectangle(e, s, out, 6.06, 2.44), 2.6, 5.2, pick(boxes, rand()), true);
    });
  } else if (yard === 2) {
    // Storage tanks on concrete pads.
    scatter(3.6, 3.6, 3, (e, s, out) => {
      const c = along(e, s, out);
      mb.prism(rectangle(e, s, out, 3.6, 3.6), 0, 0.2, concrete, true);
      const h = 4 + rand() * 3;
      mb.cylinder(c, 0.2, 1.5, h, 10, tank);
      mb.cylinder(c, 0.2 + h, 1.5, 0.6, 10, tank, 0);
    });
  } else {
    // A row of staff bays rather than a single space.
    scatter(2.8, 5, 6, bay);
  }
  // Every yard but the parking row keeps one marked bay beside the hall when it fits.
  if (yard !== 3) {
    const side = edge(b.ring, (b.front + 1) % b.ring.length);
    const spot = rectangle(side, side.L * 0.7, 4, 2.8, 5);
    place(spot, () => bay(side, side.L * 0.7, 4));
  }
  if (rand() < 0.6) {
    const back = edge(b.ring, (b.front + Math.floor(b.ring.length / 2)) % b.ring.length);
    for (const fraction of [0.85, 0.15]) {
      const shape = rectangle(back, back.L * fraction, 1.4, 2, 1.4);
      if (place(shape, () => mb.prism(shape, 0, 1.15, pick(skips, rand()), true))) break;
    }
  }
}
