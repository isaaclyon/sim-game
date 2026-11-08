// Buildings drawn in square tiles, so growth rebuilds only the tiles whose buildings changed.
// A newly grown building rises on its own mesh inside scaffolding, in fresh pale materials,
// then joins its tile. The rise is display only: the sim placed the building whole (ADR 0027).
import * as THREE from 'three';
import { buildBuildingGeometry } from './index.ts';
import { dropArraysOnUpload } from './mesh.ts';
import type { Pt } from './polygon.ts';
import { buildScaffoldGeometry } from './scaffold.ts';
import type { Style } from './style.ts';

const TILE = 256;
/** How long a new building takes to rise, in seconds. */
export const RISE_SECONDS = 9.6;
/** Fresh concrete and render, blended out as the building tops out. */
const UNFINISHED = new THREE.Color(0xe6dccb);

const clamp01 = (x: number) => Math.min(Math.max(x, 0), 1);
/**
 * Heights as a share of full and the share of finished colour, `p` of the way through a rise:
 * the scaffold goes up first, the building climbs inside it, then the scaffold comes down.
 */
export function risePhase(p: number): { scaffold: number; building: number; paint: number } {
  const climb = clamp01((p - 0.1) / 0.75);
  return {
    scaffold: Math.min(clamp01(p / 0.12), clamp01((1 - p) / 0.08)),
    // Fast at first, easing into its full height.
    building: 1 - (1 - climb) ** 3,
    paint: THREE.MathUtils.smoothstep(p, 0.55, 0.95),
  };
}

interface Rising {
  mesh: THREE.Mesh;
  scaffold: THREE.Mesh;
  /** The kit's colours, restored as the scaffold tint fades. */
  colors: Float32Array;
  start: number;
}

interface Tile {
  hash: number;
  mesh: THREE.Mesh;
  ids: number[];
  /** The tile's snapshot records, kept to rebuild one building at a time for picking. */
  records: Float32Array;
  /** Per building, in `ids` order: where its record starts in `records`. */
  starts: number[];
  /** Per building: drawn bounds `[min x, y, z, max x, y, z]`, NaN when nothing is drawn. */
  bounds: Float32Array;
}

/** Each building's drawn bounds, from its triangle range, read before the GPU takes them. */
function drawnBounds(geometry: THREE.BufferGeometry): Float32Array {
  const p = geometry.getAttribute('position').array as Float32Array;
  const index = geometry.index?.array as Uint32Array;
  const out = new Float32Array(geometry.groups.length * 6).fill(Number.NaN);
  geometry.groups.forEach(({ start, count }, b) => {
    if (count === 0) return;
    const box = out.subarray(b * 6, b * 6 + 6);
    box.set([Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity]);
    for (let i = start; i < start + count; i++)
      for (let k = 0; k < 3; k++) {
        box[k] = Math.min(box[k], p[index[i] * 3 + k]);
        box[k + 3] = Math.max(box[k + 3], p[index[i] * 3 + k]);
      }
  });
  return out;
}

export class BuildingLayer {
  readonly group = new THREE.Group();
  private readonly tiles = new Map<string, Tile>();
  private readonly rising = new Map<number, Rising>();
  private known = new Set<number>();
  private last?: { data: Float32Array; ids: Uint32Array };

  private readonly material: THREE.Material;
  private readonly style?: Style;

  constructor(material: THREE.Material, style?: Style) {
    this.material = material;
    this.style = style;
  }

  /** Picks actual roof and facade triangles, not their projection onto the ground. */
  pick(ray: THREE.Raycaster): number | null {
    this.group.updateWorldMatrix(true, true);
    let best: { distance: number; id: number } | null = null;
    for (const [id, r] of this.rising) {
      const hit = ray.intersectObjects([r.mesh, r.scaffold], false)[0];
      if (hit && (!best || hit.distance < best.distance)) best = { distance: hit.distance, id };
    }
    // Drawn tiles keep no vertices, so rebuild only the buildings whose bounds the ray enters,
    // nearest first, until the next one starts beyond the closest hit.
    const world = this.group.matrixWorld;
    const local = ray.ray.clone().applyMatrix4(world.clone().invert());
    const [box, at] = [new THREE.Box3(), new THREE.Vector3()];
    const candidates: { distance: number; tile: Tile; b: number }[] = [];
    for (const tile of this.tiles.values()) {
      const sphere = tile.mesh.geometry.boundingSphere;
      if (sphere && !local.intersectsSphere(sphere)) continue;
      for (let b = 0; b < tile.starts.length; b++) {
        if (Number.isNaN(tile.bounds[b * 6])) continue;
        // A centimetre of slack keeps rays grazing a box edge.
        box.min.fromArray(tile.bounds, b * 6).subScalar(0.01);
        box.max.fromArray(tile.bounds, b * 6 + 3).addScalar(0.01);
        if (local.intersectBox(box, at))
          candidates.push({ distance: at.applyMatrix4(world).distanceTo(ray.ray.origin), tile, b });
      }
    }
    candidates.sort((x, y) => x.distance - y.distance);
    const probe = new THREE.Mesh(undefined, this.material);
    probe.matrixWorld.copy(world);
    for (const { distance, tile, b } of candidates) {
      if (best && distance > best.distance) break;
      const o = tile.starts[b];
      const record = tile.records.subarray(o, o + 5 + 2 * tile.records[o + 4]);
      probe.geometry = buildBuildingGeometry(record, this.style);
      const hit = ray.intersectObject(probe, false)[0];
      probe.geometry.dispose();
      if (hit && (!best || hit.distance < best.distance))
        best = { distance: hit.distance, id: tile.ids[b] };
    }
    return best?.id ?? null;
  }

  /** Rebuilds every tile, whose vertices only the GPU held, after a lost WebGL context. */
  restore() {
    for (const tile of this.tiles.values()) {
      this.group.remove(tile.mesh);
      tile.mesh.geometry.dispose();
    }
    this.tiles.clear();
    if (this.last) this.update(this.last.data, this.last.ids);
  }

  /**
   * Takes the whole `[zone, height, roof, front, n, x0, y0, …]` snapshot. With `now` (seconds),
   * IDs not seen in the previous snapshot rise from that time; without it they appear at once.
   */
  update(data: Float32Array, ids: Uint32Array, now: number | null = null) {
    this.last = { data, ids };
    const present = new Set(ids);
    for (const [id, r] of this.rising) if (!present.has(id)) this.settle(id, r);
    const words = new Uint32Array(data.buffer, data.byteOffset, data.length);
    const records = new Map<string, { offset: number; id: number }[]>();
    for (let o = 0, i = 0; o < data.length; o += 5 + 2 * data[o + 4], i++) {
      if (now !== null && !this.known.has(ids[i]) && !this.rising.has(ids[i]))
        this.rise(ids[i], data.subarray(o, o + 5 + 2 * data[o + 4]), now);
      // Rising buildings join their tile once they top out.
      if (this.rising.has(ids[i])) continue;
      // A building belongs to the tile holding its first footprint point.
      const key = `${Math.floor(data[o + 5] / TILE)},${Math.floor(data[o + 6] / TILE)}`;
      const list = records.get(key);
      const record = { offset: o, id: ids[i] };
      if (list) list.push(record);
      else records.set(key, [record]);
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
      for (const { offset: o } of starts) {
        const end = o + 5 + 2 * data[o + 4];
        length += end - o;
        for (let i = o; i < end; i++) hash = Math.imul(hash ^ words[i], 0x01000193);
      }
      const tile = this.tiles.get(key);
      const tileIds = starts.map((r) => r.id);
      if (tile) tile.ids = tileIds;
      if (tile?.hash === hash) continue;
      const records = new Float32Array(length);
      const offsets: number[] = [];
      let at = 0;
      for (const { offset: o } of starts) {
        const record = data.subarray(o, o + 5 + 2 * data[o + 4]);
        offsets.push(at);
        records.set(record, at);
        at += record.length;
      }
      const geometry = buildBuildingGeometry(records, this.style);
      const bounds = drawnBounds(geometry);
      dropArraysOnUpload(geometry);
      if (tile) {
        tile.mesh.geometry.dispose();
        tile.mesh.geometry = geometry;
        Object.assign(tile, { hash, records, starts: offsets, bounds });
      } else {
        const mesh = new THREE.Mesh(geometry, this.material);
        mesh.castShadow = true;
        mesh.receiveShadow = true;
        this.group.add(mesh);
        this.tiles.set(key, { hash, mesh, ids: tileIds, records, starts: offsets, bounds });
      }
    }
    this.known = present;
  }

  /** Advances rising buildings to `now` (seconds); true while any is still rising. */
  animate(now: number): boolean {
    let settled = false;
    for (const [id, r] of this.rising) {
      const p = clamp01((now - r.start) / RISE_SECONDS);
      if (p >= 1) {
        this.settle(id, r);
        settled = true;
        continue;
      }
      const { scaffold, building, paint } = risePhase(p);
      r.scaffold.scale.y = Math.max(scaffold, 1e-3);
      r.mesh.scale.y = Math.max(building, 1e-3);
      const color = r.mesh.geometry.getAttribute('color') as THREE.InterleavedBufferAttribute;
      for (let i = 0; i < color.count; i++)
        color.setXYZ(
          i,
          UNFINISHED.r + (r.colors[3 * i] - UNFINISHED.r) * paint,
          UNFINISHED.g + (r.colors[3 * i + 1] - UNFINISHED.g) * paint,
          UNFINISHED.b + (r.colors[3 * i + 2] - UNFINISHED.b) * paint,
        );
      color.data.needsUpdate = true;
    }
    if (settled && this.last) this.update(this.last.data, this.last.ids);
    return this.rising.size > 0;
  }

  private rise(id: number, record: Float32Array, now: number) {
    const geometry = buildBuildingGeometry(record, this.style);
    const color = geometry.getAttribute('color');
    const colors = new Float32Array(color.count * 3);
    for (let i = 0; i < color.count; i++)
      colors.set([color.getX(i), color.getY(i), color.getZ(i)], 3 * i);
    // Nobody lives there yet: no lit windows until it joins its tile.
    geometry.deleteAttribute('nightEmission');
    const mesh = new THREE.Mesh(geometry, this.material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.scale.y = 1e-3;
    const ring: Pt[] = [];
    for (let i = 0; i < record[4]; i++) ring.push([record[5 + 2 * i], record[6 + 2 * i]]);
    const scaffold = new THREE.Mesh(buildScaffoldGeometry(ring, record[1]), this.material);
    scaffold.castShadow = true;
    scaffold.receiveShadow = true;
    scaffold.scale.y = 1e-3;
    this.group.add(mesh, scaffold);
    this.rising.set(id, { mesh, scaffold, colors, start: now });
  }

  private settle(id: number, r: Rising) {
    this.group.remove(r.mesh, r.scaffold);
    r.mesh.geometry.dispose();
    r.scaffold.geometry.dispose();
    this.rising.delete(id);
  }
}
