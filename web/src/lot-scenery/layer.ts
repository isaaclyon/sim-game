// Match packed snapshots spatially (neither is indexed by ID), then cache 256 m tiles.
import * as THREE from 'three';
import type { Building } from '../building-kit/kit.ts';
import { dropArraysOnUpload, MeshBuilder } from '../building-kit/mesh.ts';
import { along, ccw, edge, inside, type Pt } from '../building-kit/polygon.ts';
import { compactMaterial } from '../lighting.ts';
import { Clearance } from '../scenery/clearance.ts';
import { lotGeometry } from './geometry.ts';

const BUCKET = 64;
const TILE = 256;
interface Site {
  lot: Pt[];
  building: Building;
}
interface Tile {
  signature: string;
  group: THREE.Group;
}

export class LotLayer {
  readonly group = new THREE.Group();
  private readonly lots = new Map<string, Pt[][]>();
  private readonly tiles = new Map<string, Tile>();
  private readonly roads = new Clearance();
  // Tiles use MeshBuilder's compact format.
  private readonly material = compactMaterial(
    new THREE.MeshLambertMaterial({ vertexColors: true }),
  );
  private readonly groundMaterial = compactMaterial(
    new THREE.MeshLambertMaterial({ vertexColors: true, transparent: true, depthWrite: false }),
  );
  private buildings: Float32Array = new Float32Array();
  private roadRevision = 0;
  private dirty = false;

  setRoads(data: Float32Array) {
    this.roads.setRoads(data);
    this.roadRevision++;
    this.dirty = true;
  }

  setLots(data: Float32Array) {
    this.lots.clear();
    for (let o = 0; o < data.length; ) {
      const n = data[o++];
      const ring: Pt[] = [];
      for (let i = 0; i < n; i++, o += 2) ring.push([data[o], data[o + 1]]);
      const lot = ccw(ring);
      const xs = lot.map((p) => Math.floor(p[0] / BUCKET));
      const ys = lot.map((p) => Math.floor(p[1] / BUCKET));
      for (let x = Math.min(...xs); x <= Math.max(...xs); x++)
        for (let y = Math.min(...ys); y <= Math.max(...ys); y++) {
          const key = `${x},${y}`;
          const list = this.lots.get(key);
          if (list) list.push(lot);
          else this.lots.set(key, [lot]);
        }
    }
    this.dirty = true;
  }

  setBuildings(data: Float32Array) {
    this.buildings = data;
    this.dirty = true;
  }

  /** Coalesce roads, lots and buildings from one worker edit before rebuilding. */
  update() {
    if (!this.dirty) return;
    this.dirty = false;
    const sites = new Map<string, Site[]>();
    const data = this.buildings;
    for (let o = 0; o < data.length; ) {
      const [zone, height, roof, front, era, n] = data.subarray(o, o + 6);
      o += 6;
      const ring: Pt[] = [];
      for (let i = 0; i < n; i++, o += 2) ring.push([data[o], data[o + 1]]);
      // Just inside the first edge works even for concave footprints whose centroid is outside.
      const e = edge(ring, 0);
      const p = along(e, e.L / 2, -0.01);
      const candidates = this.lots.get(`${Math.floor(p[0] / BUCKET)},${Math.floor(p[1] / BUCKET)}`);
      const lot = candidates?.find((r) => inside(p, r));
      if (!lot) continue;
      const key = `${Math.floor(lot[0][0] / TILE)},${Math.floor(lot[0][1] / TILE)}`;
      const site = { lot, building: { zone, height, roof, front, era, ring } };
      const list = sites.get(key);
      if (list) list.push(site);
      else sites.set(key, [site]);
    }
    for (const [key, tile] of this.tiles) {
      if (!sites.has(key)) {
        this.dispose(tile);
        this.tiles.delete(key);
      }
    }
    for (const [key, list] of sites) {
      const signature = `${this.roadRevision}:${JSON.stringify(list)}`;
      const old = this.tiles.get(key);
      if (old?.signature === signature) continue;
      if (old) this.dispose(old);
      const props = new MeshBuilder({ compact: true });
      const ground = new MeshBuilder({ compact: true });
      for (const { lot, building } of list) lotGeometry(props, lot, building, this.roads, ground);
      const group = new THREE.Group();
      const [below, above] = [ground.geometry(), props.geometry()];
      // Nothing reads lot scenery back, and restore() rebuilds it.
      dropArraysOnUpload(below);
      dropArraysOnUpload(above);
      const surface = new THREE.Mesh(below, this.groundMaterial);
      surface.renderOrder = -5; // Above terrain, below zones, sidewalks and roads.
      group.add(surface, new THREE.Mesh(above, this.material));
      this.group.add(group);
      this.tiles.set(key, { signature, group });
    }
  }

  /** Rebuilds every tile, whose vertices only the GPU held, after a lost WebGL context. */
  restore() {
    for (const tile of this.tiles.values()) this.dispose(tile);
    this.tiles.clear();
    this.dirty = true;
    this.update();
  }

  private dispose(tile: Tile) {
    this.group.remove(tile.group);
    for (const mesh of tile.group.children) (mesh as THREE.Mesh).geometry.dispose();
  }
}
