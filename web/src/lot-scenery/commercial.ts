// Coherent, footprint-seeded shop frontages and rear spaces. Nothing here confers parking,
// seating capacity or trade; the sim owns jobs and visits.
import * as THREE from 'three';
import { type Building, seeded } from '../building-kit/kit.ts';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { along, type Edge, edge, inside, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { fits, frontage, rectangle } from './placement.ts';
import { bench, tree } from './props.ts';

const lawns = [0x8fa66c, 0x98ad75, 0x849c64, 0x9aab79];
const pavings = [0xc8bda8, 0xb3b0a6, 0xa9826e]; // Sand, grey slabs, brick.
const asphalt = new THREE.Color(0x858b87);
const concrete = new THREE.Color(0xaaa99e);
const paint = new THREE.Color(0xd4cdb7);
const hedge = new THREE.Color(0x56794d);
const timber = new THREE.Color(0xa58a65);
const metal = new THREE.Color(0x5f676b);
const bollardGrey = new THREE.Color(0x4a4f52);
const tableTop = new THREE.Color(0xe4ddcc);
const canopies = [0xd9cfb8, 0x9a4f4f, 0x4f6f86];
const bins = [0x3f5a46, 0x6b6f72];
const stock = new THREE.Color(0xb4996e);
const shed = new THREE.Color(0x8c9396);
const shedRoof = new THREE.Color(0x6c7377);
const shedDoor = new THREE.Color(0x4f5558);
/** Set-back shops keep this much clear asphalt along their front, then mark bays beyond it. */
const SIDEWALK = 1.6;
const BAY = 2.6;
const pick = (list: number[], r: number) => new THREE.Color(list[Math.floor(r * list.length)]);

/** A pallet of boxed stock centred `out` from `e`. Needs 1.2 m square. */
function pallet(mb: MeshBuilder, e: Edge, s: number, out: number, height: number) {
  mb.prism(rectangle(e, s, out, 1.2, 1.2), 0, 0.14, timber, true);
  mb.prism(rectangle(e, s, out, 1.1, 1.1), 0.14, height, stock, true);
}

/** A round table with two chairs across it, centred `out` from `e`. Needs 1.6 × 1.2 m. */
function table(mb: MeshBuilder, e: Edge, s: number, out: number, canopy: THREE.Color | null) {
  const c = along(e, s, out);
  mb.cylinder(c, 0, 0.05, 0.7, 5, metal);
  mb.cylinder(c, 0.7, 0.38, 0.05, 8, tableTop);
  for (const d of [-0.6, 0.6]) {
    mb.prism(rectangle(e, s + d, out, 0.4, 0.4), 0, 0.45, timber, true);
    mb.prism(rectangle(e, s + d * 1.25, out, 0.08, 0.4), 0.45, 0.85, timber, true);
  }
  if (!canopy) return;
  mb.cylinder(c, 0.75, 0.03, 1.4, 4, metal);
  mb.cylinder(c, 1.95, 1.1, 0.45, 8, canopy, 0);
}

export function commercialGeometry(
  mb: MeshBuilder,
  surface: MeshBuilder,
  lot: Pt[],
  b: Building,
  roads: Clearance,
) {
  // The building kit seeds from the first two points; offset to keep choices independent.
  const rand = seeded(b.ring.slice(1));
  const small = b.zone === 3;
  const paving = pick(pavings, rand());
  const frontKind = Math.floor(rand() * 4);
  // Storage is the most common rear use. Dense blocks have no room for trees or terraces.
  const rearKind = small ? [0, 1, 2, 2, 3][Math.floor(rand() * 5)] : rand() < 0.5 ? 2 : 3;
  surface.poly(lot, 0, small ? pick(lawns, rand()) : paving);

  const front = edge(b.ring, b.front);
  // Small shops have a door 30% along; dense blocks are glazed across their whole front.
  const door = small ? front.L * 0.3 : front.L / 2;
  const walk = rectangle(front, door, 30, small ? 2.4 : front.L * 0.5, 60);
  const occupied: Pt[][] = [b.ring, walk];
  // Everything between a small shop and the street is paved or parked on, never left as
  // bare grass. Dense blocks already stand on paving.
  const forecourt = small ? frontage(lot, front, roads) : null;
  const depth = forecourt
    ? Math.max(
        ...forecourt.map(
          (p) => (p[0] - front.a[0]) * front.n[0] + (p[1] - front.a[1]) * front.n[1],
        ),
      )
    : 0;

  const place = (shape: Pt[], draw: () => void) => {
    if (!fits(shape, lot, occupied, roads)) return false;
    draw();
    occupied.push(shape);
    return true;
  };
  // Spots along the frontage on either side of the entrance, nearest the door first.
  const spots = (width: number) => {
    const out: number[] = [];
    for (let s = width / 2 + 0.3; s <= front.L - width / 2 - 0.3; s += width + 0.3) out.push(s);
    return out.sort((x, y) => Math.abs(x - door) - Math.abs(y - door));
  };
  const planters = () => {
    let n = 0;
    for (const fraction of [0.1, 0.9]) {
      const shape = rectangle(front, front.L * fraction, 0.4, 1.1, 0.35);
      const drawn = place(shape, () => {
        mb.prism(shape, 0, 0.35, concrete, true);
        mb.prism(rectangle(front, front.L * fraction, 0.4, 0.95, 0.25), 0.35, 0.65, hedge, true);
      });
      if (drawn) n++;
    }
    return n;
  };

  // Shops set well back from the street park customers in front, either side of the walkway.
  // Shops set well back park cars in front: one asphalt car park out to the street, with bays
  // either side of the door wherever a whole bay fits on the lot.
  const parkFront = (area: Pt[]) => {
    const on = (shape: Pt[]) => shape.every((p) => inside(p, area));
    const bays: number[] = [];
    for (const [from, to] of [
      [0.3, door - 1.2],
      [door + 1.2, front.L - 0.3],
    ]) {
      const n = Math.floor((to - from) / BAY);
      for (let k = 0; k < n; k++) {
        const c = from + (to - from - n * BAY) / 2 + (k + 0.5) * BAY;
        if (on(rectangle(front, c, SIDEWALK + 2.5, BAY + 0.12, 4.8))) bays.push(c);
      }
    }
    if (!bays.length) return 0;
    surface.poly(area, 0, asphalt);
    occupied.push(area);
    for (const c of bays) {
      for (const d of [-BAY / 2, BAY / 2])
        surface.poly(rectangle(front, c + d, SIDEWALK + 2.5, 0.12, 4.8), 0, paint);
      mb.prism(rectangle(front, c, SIDEWALK + 0.35, 1.8, 0.16), 0.01, 0.14, concrete, true);
    }
    return bays.length;
  };
  const frontProps = () => {
    let placed = 0;
    if (frontKind === 0) placed = planters();
    else if (frontKind === 1) {
      // Pavement café tables, under the awning, so they need no parasols.
      for (const s of spots(1.6).slice(0, 3))
        if (place(rectangle(front, s, 1, 1.6, 1.2), () => table(mb, front, s, 1, null))) placed++;
    } else if (frontKind === 2) {
      // A short row of cycle hoops, and a bench where it fits.
      for (const s of spots(0.9).slice(0, 4)) {
        const shape = rectangle(front, s, 0.6, 0.12, 0.8);
        const hoop = place(shape, () => {
          for (const d of [-0.3, 0.3])
            mb.cylinder(along(front, s, 0.6 + d), 0, 0.03, 0.8, 4, metal);
          mb.prism(rectangle(front, s, 0.6, 0.06, 0.66), 0.8, 0.86, metal, true);
        });
        if (hoop) placed++;
      }
      const s = spots(2).at(-1);
      if (s !== undefined) place(rectangle(front, s, 0.8, 2, 0.6), () => bench(mb, front, s, 0.8));
    } else {
      // Bollards guard the frontage.
      for (const s of spots(0.3)) {
        if (Math.round(s / 0.6) % 3) continue;
        const shape = rectangle(front, s, 0.5, 0.3, 0.3);
        if (place(shape, () => mb.cylinder(along(front, s, 0.5), 0, 0.12, 0.9, 6, bollardGrey)))
          placed++;
      }
    }
    return placed;
  };
  const parked = forecourt && depth >= SIDEWALK + 5.4 ? parkFront(forecourt) : 0;
  if (forecourt && !parked) surface.poly(forecourt, 0, paving);
  const placed = parked || frontProps();
  // A cramped frontage that fits none of the chosen kind still gets its planters.
  if (!placed && frontKind !== 0) planters();

  const side = edge(b.ring, (b.front + 1) % b.ring.length);
  // A single marked bay, not a sea of parking. No parked vehicle or capacity is implied.
  const bay = rectangle(side, side.L * 0.7, 4, 2.8, 5);
  place(bay, () => {
    surface.poly(bay, 0, asphalt);
    for (const s of [-1.25, 1.25])
      surface.poly(rectangle(side, side.L * 0.7 + s, 4, 0.12, 4.8), 0, paint);
    surface.poly(rectangle(side, side.L * 0.7, 1.65, 2.5, 0.12), 0, paint);
  });

  const back = edge(b.ring, (b.front + Math.floor(b.ring.length / 2)) % b.ring.length);
  const mid = back.L / 2;
  if (rearKind === 0) {
    // A garden of one or two trees.
    for (const fraction of rand() < 0.5 ? [0.5] : [0.25, 0.75]) {
      const s = back.L * fraction;
      place(rectangle(back, s, 5, 3.6, 3.6), () => tree(mb, along(back, s, 5), rand));
    }
  } else if (rearKind === 1 && back.L >= 4) {
    // A paved terrace with parasol tables.
    const width = Math.min(back.L - 1, 9);
    const terrace = rectangle(back, mid, 3.4, width, 5.2);
    place(terrace, () => {
      surface.poly(terrace, 0, paving);
      const canopy = pick(canopies, rand());
      const count = Math.max(1, Math.floor(width / 3));
      for (let k = 0; k < count; k++) table(mb, back, mid + (k - (count - 1) / 2) * 3, 3.6, canopy);
    });
  } else if (rearKind === 2) {
    // A storage yard with a shed and stacked stock where it fits, else a bin corner.
    const width = Math.min(back.L - 1, 10);
    const yard = rectangle(back, mid, 3.4, width, 6);
    const stored =
      small &&
      width >= 8 &&
      place(yard, () => {
        surface.poly(yard, 0, concrete);
        const s = mid - width / 2 + 1.8;
        mb.prism(rectangle(back, s, 4.6, 3, 2.4), 0, 2.3, shed, true);
        mb.prism(rectangle(back, s, 4.6, 3.3, 2.7), 2.3, 2.42, shedRoof, true);
        const inward: Pt = [-back.n[0], -back.n[1]];
        mb.wall(along(back, s + 0.6, 3.38), along(back, s - 0.6, 3.38), 0, 2, shedDoor, inward);
        for (const d of [-0.2, 1]) {
          const bin = rectangle(back, mid + d, 1.3, 1, 0.8);
          mb.prism(bin, 0, 1.2, pick(bins, rand()), true);
        }
        const s2 = mid + width / 2 - 1;
        for (const out of [2, 3.6, 5.2]) pallet(mb, back, s2, out, 0.6 + rand() * 0.9);
      });
    const pad = rectangle(back, mid, 1.9, 6, 2.6);
    if (!stored)
      place(pad, () => {
        surface.poly(pad, 0, concrete);
        for (const d of [-2, -0.8])
          mb.prism(rectangle(back, mid + d, 1.6, 1, 0.8), 0, 1.2, pick(bins, rand()), true);
        pallet(mb, back, mid + 1.4, 1.9, 0.9);
      });
  } else if (small) {
    // An open lawn with a bench to sit out on.
    place(rectangle(back, mid, 3, 2, 0.6), () => bench(mb, back, mid, 3));
  }
}
