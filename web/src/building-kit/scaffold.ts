// Display-only scaffolding around a building while it rises: steel standards along every
// footprint edge, a timber deck and guard rail at each lift, and a diagonal brace per face.
import * as THREE from 'three';
import { MeshBuilder } from './mesh.ts';
import { along, ccw, edge, type Pt } from './polygon.ts';

const STEEL = new THREE.Color(0x9aa0a6);
const TIMBER = new THREE.Color(0xc89a5a);
const RAIL = new THREE.Color(0xe0b23a);
/** Metres: how far the outer standards stand from the wall, lift height, bay length. */
const OUT = 1.2;
const LIFT = 2;
const BAY = 2.5;
const POLE = 0.12;

/** Scaffolding for a counter-clockwise footprint `ring` (sim x, y) up to `height` metres. */
export function buildScaffoldGeometry(ring: Pt[], height: number): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  if (ring.length < 3 || ring !== ccw(ring)) return mb.geometry();
  const top = height + 1;
  for (let i = 0; i < ring.length; i++) {
    const e = edge(ring, i);
    if (e.L < 1) continue;
    const rot = Math.atan2(-e.u[1], e.u[0]);
    const bays = Math.max(1, Math.round(e.L / BAY));
    for (let k = 0; k <= bays; k++)
      mb.box(along(e, (e.L * k) / bays, OUT), 0, POLE, top, POLE, rot, STEEL);
    for (let y = LIFT; y <= height + 0.01; y += LIFT) {
      mb.box(along(e, e.L / 2, OUT / 2 + 0.1), y, e.L + OUT, 0.08, OUT - 0.1, rot, TIMBER);
      mb.box(along(e, e.L / 2, OUT), y + 1, e.L, 0.08, 0.08, rot, RAIL);
    }
    // One brace across the face, from the first standard's foot to the last one's top lift.
    const [a, b] = [along(e, 0, OUT + 0.08), along(e, e.L, OUT + 0.08)];
    const [lo, hi, w] = [0.2, Math.max(height, LIFT), 0.1];
    mb.quad(
      [a[0], lo, a[1]],
      [b[0], hi - w, b[1]],
      [b[0], hi + w, b[1]],
      [a[0], lo + 2 * w, a[1]],
      STEEL,
      [e.n[0], 0, e.n[1]],
    );
  }
  return mb.geometry();
}
