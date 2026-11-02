// Decorative land drawn under the city: ground, water, fields and trees. Wild scenery gives
// way to roads, zones and lots as they appear; it never changes what the sim does (issue #27).
import * as THREE from 'three';
import { Clearance } from './clearance.ts';
import { type Field, Land } from './land.ts';
import { fieldsGeometry, groundGeometry, waterGeometry } from './terrain.ts';
import { streetTrees, type Tree, TreeLayer, wildTrees } from './trees.ts';

/** Wild trees and fields keep this far from asphalt: a sidewalk plus a verge. */
const WILD_MARGIN = 5;
const GROUND_CELL = 40;

export class Scenery {
  readonly group = new THREE.Group();
  private readonly clearance = new Clearance();
  private land?: Land;
  private wild: Tree[] = [];
  private segments: Float32Array = new Float32Array();
  private fields?: THREE.Mesh;
  private trees?: TreeLayer;
  private dirty = false;

  /** Lays out the land once, over `[x0, y0, x1, y1]` in sim metres. */
  setExtent(extent: [number, number, number, number]) {
    this.group.clear();
    const land = new Land(extent);
    this.land = land;
    const material = new THREE.MeshLambertMaterial({ vertexColors: true, depthWrite: false });
    // Flat layers share the ground plane: no depth writes, drawn in this order before
    // anything else, so roads and the zone overlay cover them.
    for (const [i, geometry] of [groundGeometry(land, GROUND_CELL), waterGeometry(land)].entries())
      this.group.add(flat(geometry, material, -10 + i));
    this.fields = flat(new THREE.BufferGeometry(), material, -8);
    this.group.add(this.fields);
    this.wild = wildTrees(land);
    this.trees = new TreeLayer(this.wild.length + 20000);
    this.group.add(this.trees.group);
    this.dirty = true;
  }

  setRoads(segments: Float32Array) {
    this.segments = segments;
    this.clearance.setRoads(segments);
    this.dirty = true;
  }

  setZones(cells: Uint8Array, layout: [number, number, number, number, number]) {
    this.clearance.setZones(cells, layout);
    this.dirty = true;
  }

  setLots(rings: Float32Array) {
    this.clearance.setLots(rings);
    this.dirty = true;
  }

  /** Rebuilds what the latest roads, zones and lots cleared; call once per frame. */
  update() {
    if (!this.dirty || !this.land || !this.trees || !this.fields) return;
    this.dirty = false;
    const c = this.clearance;
    const open = (x: number, y: number) => !c.blocked(x, y, WILD_MARGIN);
    const fields = this.land.fields.filter((f: Field) =>
      [...f.corners, centre(f)].every(([x, y]) => open(x, y)),
    );
    this.fields.geometry.dispose();
    this.fields.geometry = fieldsGeometry(fields);
    const kept = this.wild.filter((t) => open(t.x, t.y));
    this.trees.set([...kept, ...streetTrees(this.segments, c)]);
  }
}

function centre(f: Field): [number, number] {
  const [a, , c] = f.corners;
  return [(a[0] + c[0]) / 2, (a[1] + c[1]) / 2];
}

function flat(geometry: THREE.BufferGeometry, material: THREE.Material, order: number) {
  const mesh = new THREE.Mesh(geometry, material);
  mesh.renderOrder = order;
  mesh.frustumCulled = false;
  return mesh;
}
