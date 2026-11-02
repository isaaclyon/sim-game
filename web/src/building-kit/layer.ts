// Buildings drawn in square tiles, so growth rebuilds only the tiles whose buildings changed.
import * as THREE from 'three';
import { buildBuildingGeometry } from './index.ts';
import type { Style } from './style.ts';

const TILE = 256;

interface Tile {
  hash: number;
  mesh: THREE.Mesh;
}

export class BuildingLayer {
  readonly group = new THREE.Group();
  private readonly tiles = new Map<string, Tile>();

  private readonly material: THREE.Material;
  private readonly style?: Style;

  constructor(material: THREE.Material, style?: Style) {
    this.material = material;
    this.style = style;
  }

  /** Takes the whole `[zone, height, roof, front, n, x0, y0, …]` snapshot. */
  update(data: Float32Array) {
    const words = new Uint32Array(data.buffer, data.byteOffset, data.length);
    const records = new Map<string, number[]>();
    for (let o = 0; o < data.length; o += 5 + 2 * data[o + 4]) {
      // A building belongs to the tile holding its first footprint point.
      const key = `${Math.floor(data[o + 5] / TILE)},${Math.floor(data[o + 6] / TILE)}`;
      const list = records.get(key);
      if (list) list.push(o);
      else records.set(key, [o]);
    }
    for (const [key, tile] of this.tiles)
      if (!records.has(key)) {
        this.group.remove(tile.mesh);
        tile.mesh.geometry.dispose();
        this.tiles.delete(key);
      }
    for (const [key, starts] of records) {
      // FNV-1a over the raw float bits of this tile's records, in snapshot order.
      let hash = 0x811c9dc5;
      let length = 0;
      for (const o of starts) {
        const end = o + 5 + 2 * data[o + 4];
        length += end - o;
        for (let i = o; i < end; i++) hash = Math.imul(hash ^ words[i], 0x01000193);
      }
      const tile = this.tiles.get(key);
      if (tile?.hash === hash) continue;
      const subset = new Float32Array(length);
      let at = 0;
      for (const o of starts) {
        const record = data.subarray(o, o + 5 + 2 * data[o + 4]);
        subset.set(record, at);
        at += record.length;
      }
      const geometry = buildBuildingGeometry(subset, this.style);
      if (tile) {
        tile.mesh.geometry.dispose();
        tile.mesh.geometry = geometry;
        tile.hash = hash;
      } else {
        const mesh = new THREE.Mesh(geometry, this.material);
        this.group.add(mesh);
        this.tiles.set(key, { hash, mesh });
      }
    }
  }
}
