// Small outdoor props shared by several lot uses.
import * as THREE from 'three';
import type { MeshBuilder } from '../building-kit/mesh.ts';
import type { Edge, Pt } from '../building-kit/polygon.ts';
import { rectangle } from './placement.ts';

const timber = new THREE.Color(0xa58a65);
const concrete = new THREE.Color(0xaaa99e);
const leaves = [0x6b9054, 0x7e9b5e, 0x527c51];

/** A low-poly tree; its crown height and colour come from `rand`. Needs about 3.6 m square. */
export function tree(mb: MeshBuilder, c: Pt, rand: () => number) {
  mb.cylinder(c, 0, 0.17, 2.2, 5, timber);
  const crown = 2.5 + rand();
  mb.cylinder(
    c,
    1.5,
    1.65,
    crown,
    7,
    new THREE.Color(leaves[Math.floor(rand() * leaves.length)]),
    0.25,
  );
}

/** A slatted bench `out` metres from edge `e`, its back towards the edge. Needs 2 × 0.6 m. */
export function bench(mb: MeshBuilder, e: Edge, s: number, out: number) {
  for (const d of [-0.7, 0.7])
    mb.prism(rectangle(e, s + d, out, 0.15, 0.5), 0, 0.45, concrete, true);
  mb.prism(rectangle(e, s, out, 2, 0.6), 0.45, 0.58, timber, true);
  mb.prism(rectangle(e, s, out - 0.25, 2, 0.12), 0.58, 1.05, timber, true);
}
