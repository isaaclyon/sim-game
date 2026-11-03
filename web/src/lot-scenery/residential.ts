// Coherent, footprint-seeded house gardens. None of these spaces confer parking or capacity.
import * as THREE from 'three';
import { type Building, houseDoorAt, seeded } from '../building-kit/kit.ts';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import { along, type Edge, edge, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';
import { type BoundaryStyle, boundary } from './boundary.ts';
import { access, fits, reach, rectangle } from './placement.ts';

const paving = new THREE.Color(0xc8bda8);
const asphalt = new THREE.Color(0x858b87);
const concrete = new THREE.Color(0xaaa99e);
const timber = new THREE.Color(0xa58a65);
const soil = new THREE.Color(0x76644c);
const green = new THREE.Color(0x56794d);
const flowers = [0xc88791, 0xe0c077, 0xb4a0c7];
const leaves = [0x6b9054, 0x7e9b5e, 0x527c51];

export function residentialGeometry(
  mb: MeshBuilder,
  surface: MeshBuilder,
  lot: Pt[],
  b: Building,
  roads: Clearance,
) {
  const rand = seeded(b.ring);
  const enclosure: BoundaryStyle = (['hedge', 'picket', 'none'] as const)[Math.floor(rand() * 3)];
  let frontKind = Math.floor(rand() * 3);
  const backKind = Math.floor(rand() * 4);
  const flower = new THREE.Color(flowers[Math.floor(rand() * flowers.length)]);
  const front = edge(b.ring, b.front);
  const occupied = [b.ring];
  const path = access(lot, front, houseDoorAt(b), 1.6, 0.08, occupied, roads);
  if (path) {
    surface.poly(path, 0, paving);
    occupied.push(path);
  }

  if (frontKind === 1) {
    // Prefer parking beside the house. Deep front yards can instead fit a marked forecourt.
    // A wheel stop, not a fictional garage door, explains where the driveway ends.
    let parked = false;
    for (const [s, start] of [
      [-1.8, -4.8],
      [front.L + 1.8, -4.8],
      [1.6, 0.9],
      [front.L - 1.6, 0.9],
    ]) {
      const drive = access(lot, front, s, 2.8, start, occupied, roads);
      if (!drive) continue;
      const depth = Math.hypot(drive[3][0] - drive[0][0], drive[3][1] - drive[0][1]);
      if (depth < 5.4) continue;
      surface.poly(drive, 0, asphalt);
      occupied.push(drive);
      mb.prism(rectangle(front, s, start + 0.3, 2.3, 0.16), 0.01, 0.14, concrete, true);
      parked = true;
      break;
    }
    if (!parked) frontKind = 0;
  }

  // Reserve complete boundaries after access, before any loose planting or furniture.
  occupied.push(...boundary(mb, lot, occupied, roads, enclosure));
  const place = (shape: Pt[], draw: () => void) => {
    if (!fits(shape, lot, occupied, roads)) return false;
    draw();
    occupied.push(shape);
    return true;
  };
  const tree = (e: Edge, s: number, out: number) => {
    const shape = rectangle(e, s, out, 3.6, 3.6);
    place(shape, () => {
      const c = along(e, s, out);
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
  };
  const bed = (
    e: Edge,
    s: number,
    out: number,
    width: number,
    depth: number,
    vegetables: boolean,
  ) => {
    place(rectangle(e, s, out, width, depth), () => {
      mb.prism(rectangle(e, s, out, width, depth), 0, 0.12, timber, true);
      mb.poly(rectangle(e, s, out, width - 0.16, depth - 0.16), 0.13, soil);
      for (const across of [-0.3, 0, 0.3])
        for (const row of [-0.25, 0.25]) {
          const c = along(e, s + across * width, out + row * depth);
          mb.cylinder(c, 0.13, 0.22, vegetables ? 0.35 : 0.26, 5, green, 0.16);
          if (!vegetables) mb.cylinder(c, 0.39, 0.18, 0.12, 5, flower, 0.13);
        }
    });
  };

  const frontDepth = reach(lot, front, houseDoorAt(b), 0);
  if (frontKind === 0) {
    const deep = frontDepth > 7;
    for (const fraction of [0.2, 0.8])
      bed(front, front.L * fraction, deep ? 3 : 1.5, deep ? 3.2 : 2.6, deep ? 2 : 1, false);
    if (deep) bed(front, front.L * 0.2, frontDepth * 0.7, 3.2, 1.6, false);
  } else if (frontKind === 2 && frontDepth > 5.2) {
    tree(front, front.L * 0.2, Math.min(frontDepth / 2, 4));
  }

  const back = edge(b.ring, (b.front + Math.floor(b.ring.length / 2)) % b.ring.length);
  if (backKind === 1) {
    const patio = rectangle(back, back.L / 2, 2.4, 4.8, 3.8);
    place(patio, () => {
      surface.poly(patio, 0, paving);
      for (const s of [-0.45, 0.45])
        mb.prism(rectangle(back, back.L / 2 + s, 2.4, 0.12, 0.7), 0, 0.65, concrete, true);
      mb.prism(rectangle(back, back.L / 2, 2.4, 1.4, 0.9), 0.65, 0.77, timber, true);
      for (const out of [1, 3.8]) {
        for (const s of [-0.7, 0.7])
          mb.prism(rectangle(back, back.L / 2 + s, out, 0.12, 0.4), 0, 0.42, concrete, true);
        mb.prism(rectangle(back, back.L / 2, out, 2, 0.5), 0.42, 0.54, timber, true);
      }
    });
  } else if (backKind === 2) {
    for (const fraction of [0.3, 0.7]) bed(back, back.L * fraction, 2.5, 2.2, 3, true);
  } else if (backKind === 3) {
    tree(back, back.L / 2, 4);
  }
}
