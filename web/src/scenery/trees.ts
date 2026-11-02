// Tree placement and instanced drawing. Wild trees come from the land; street trees line
// roads in residential and commercial zones. All of it is decoration (issue #27).
import * as THREE from 'three';
import { TREES } from '../palette.ts';
import type { Clearance } from './clearance.ts';
import type { Land } from './land.ts';
import { hash2, rng } from './noise.ts';

export type Kind = 'round' | 'conifer' | 'street';

export interface Tree {
  x: number;
  y: number;
  kind: Kind;
  scale: number;
}

/** Woods, meadow scatter and hedgerows, before anything clears them. */
export function wildTrees(land: Land): Tree[] {
  const [x0, y0, x1, y1] = land.extent;
  const r = rng(4242);
  const out: Tree[] = [];
  for (let y = y0; y < y1; y += 16)
    for (let x = x0; x < x1; x += 16) {
      const [px, py] = [x + (r() - 0.5) * 13, y + (r() - 0.5) * 13];
      if (land.woods(px, py)) {
        const kind = r() < 0.3 ? 'conifer' : 'round';
        out.push({ x: px, y: py, kind, scale: 0.8 + r() * 0.5 });
      }
    }
  for (let y = y0; y < y1; y += 45)
    for (let x = x0; x < x1; x += 45) {
      const [px, py] = [x + r() * 40, y + r() * 40];
      if (r() < 0.14 && !land.woods(px, py) && land.waterDistance(px, py) > 8)
        out.push({ x: px, y: py, kind: 'round', scale: 0.7 + r() * 0.5 });
    }
  for (const f of land.fields)
    for (const [x, y] of f.hedge) out.push({ x, y, kind: 'round', scale: 0.6 + r() * 0.35 });
  return out.filter((t) => land.waterDistance(t.x, t.y) > 4);
}

/** Asphalt half-widths by road class; street trees stand 1.2 m beyond, on the sidewalk. */
const ASPHALT = [4, 6, 4];

/** One tree every 15 m along streets and main roads that front residential or commercial zones. */
export function streetTrees(segments: Float32Array, clearance: Clearance): Tree[] {
  const out: Tree[] = [];
  for (let i = 0; i < segments.length; i += 5) {
    const [ax, ay, bx, by, cls] = segments.subarray(i, i + 5);
    if (cls === 2) continue;
    const l = Math.hypot(bx - ax, by - ay);
    if (l < 8) continue;
    const [ux, uy] = [(bx - ax) / l, (by - ay) / l];
    const half = ASPHALT[cls] + 1.2;
    for (let s = 7; s < l - 4; s += 15)
      for (const side of [-1, 1]) {
        const [x, y] = [ax + ux * s - uy * half * side, ay + uy * s + ux * half * side];
        const behind: [number, number] = [x - uy * 8 * side, y + ux * 8 * side];
        if (!clearance.residentialOrCommercial(...behind) || clearance.roadGap(x, y) < 1) continue;
        if (clearance.inLot(x, y) || hash2(Math.round(x), Math.round(y)) < 0.15) continue;
        out.push({ x, y, kind: 'street', scale: 0.85 + hash2(Math.round(y), Math.round(x)) * 0.3 });
      }
  }
  return out;
}

const UP = new THREE.Vector3(0, 1, 0);

/** Three instanced meshes (trunks, round canopies, conifers) rebuilt as one batch. */
export class TreeLayer {
  readonly group = new THREE.Group();
  private readonly trunks: THREE.InstancedMesh;
  private readonly canopies: THREE.InstancedMesh;
  private readonly cones: THREE.InstancedMesh;

  constructor(capacity: number) {
    const material = new THREE.MeshLambertMaterial({ flatShading: true });
    const trunk = new THREE.CylinderGeometry(0.22, 0.22, 1, 5).translate(0, 0.5, 0);
    const canopy = new THREE.IcosahedronGeometry(1, 0);
    const cone = new THREE.ConeGeometry(1, 1, 6).translate(0, 0.5, 0);
    this.trunks = new THREE.InstancedMesh(trunk, material, capacity);
    this.canopies = new THREE.InstancedMesh(canopy, material, capacity);
    this.cones = new THREE.InstancedMesh(cone, material, capacity * 2);
    for (const mesh of [this.trunks, this.canopies, this.cones]) {
      mesh.frustumCulled = false;
      mesh.count = 0;
      this.group.add(mesh);
    }
  }

  get capacity(): number {
    return this.trunks.instanceMatrix.count;
  }

  set(trees: Tree[]) {
    const [m, q, p, s, c] = [
      new THREE.Matrix4(),
      new THREE.Quaternion(),
      new THREE.Vector3(),
      new THREE.Vector3(),
      new THREE.Color(),
    ];
    const n = { trunks: 0, canopies: 0, cones: 0 };
    const add = (mesh: THREE.InstancedMesh, key: keyof typeof n, color: number) => {
      if (n[key] >= mesh.instanceMatrix.count) return;
      m.compose(p, q, s);
      mesh.setMatrixAt(n[key], m);
      mesh.setColorAt(n[key], c.set(color));
      n[key]++;
    };
    for (const t of trees.slice(0, this.capacity)) {
      const r = hash2(Math.floor(t.x * 13.1), Math.floor(t.y * 7.7));
      if (t.kind === 'conifer') {
        const [h, rad] = [9 * t.scale, 2.4 * t.scale];
        q.identity();
        [p.set(t.x, 0, t.y), s.set(1, 1.6 * t.scale, 1)];
        add(this.trunks, 'trunks', TREES.trunk);
        q.setFromAxisAngle(UP, r * 3);
        [p.set(t.x, 1.2 * t.scale, t.y), s.set(rad, h * 0.62, rad)];
        add(this.cones, 'cones', TREES.conifer);
        [p.set(t.x, 1.2 * t.scale + h * 0.38, t.y), s.set(rad * 0.72, h * 0.55, rad * 0.72)];
        add(this.cones, 'cones', TREES.conifer);
        continue;
      }
      const street = t.kind === 'street';
      const trunkH = (street ? 2.4 : 2) * t.scale;
      const rad = (street ? 2 : 2.7) * t.scale * (0.85 + r * 0.3);
      q.identity();
      [p.set(t.x, 0, t.y), s.set(1, trunkH + rad * 0.5, 1)];
      add(this.trunks, 'trunks', TREES.trunk);
      q.setFromEuler(new THREE.Euler(r * 2, r * 5, r * 3));
      [p.set(t.x, trunkH + rad * 0.85, t.y), s.set(rad, rad * (0.85 + r * 0.15), rad)];
      add(this.canopies, 'canopies', street ? TREES.street : TREES.canopy[Math.floor(r * 3)]);
    }
    for (const [mesh, key] of [
      [this.trunks, 'trunks'],
      [this.canopies, 'canopies'],
      [this.cones, 'cones'],
    ] as const) {
      mesh.count = n[key];
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
  }
}
