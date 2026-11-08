// Flat-shaded, vertex-coloured triangles in scene-local space (x, height, z).
import * as THREE from 'three';
import type { Pt } from './polygon.ts';

export type V3 = [number, number, number];

const UP: V3 = [0, 1, 0];

/** Unit float in [0, 1] to a normalized integer up to `max`; three.js's own conversion wraps. */
const unit = (v: number, max: number) => Math.round(Math.min(Math.max(v, 0), 1) * max);

/** Three values per vertex, padded to four so GPUs read them without converting the buffer. */
function packed(array: Int8Array | Uint8Array | Uint16Array): THREE.InterleavedBufferAttribute {
  return new THREE.InterleavedBufferAttribute(new THREE.InterleavedBuffer(array, 4), 3, 0, true);
}

/**
 * Lets the page drop `geometry`'s vertex arrays once the renderer has copied them to the GPU, so
 * a city's tiles are not held twice. Raycasting such a mesh then fails, and after a lost WebGL
 * context its owner must rebuild it. Bounding spheres, computed beforehand, still cull it.
 */
export function dropArraysOnUpload(geometry: THREE.BufferGeometry) {
  const { attributes, index } = geometry;
  for (const attribute of [...Object.values(attributes), ...(index ? [index] : [])])
    ('data' in attribute ? attribute.data : attribute).onUpload(function (this: {
      array: unknown;
    }) {
      this.array = null;
    });
}

/**
 * Vertices are stored compactly, since a city holds tens of millions of them: float positions,
 * byte normals and glow, 16-bit colours (bytes would lose dark palette colours, which are linear
 * here) and half-float light hours. Glow and hours are left out when no face has any; the night
 * shader then reads zeros. Faces are flat, so vertices are shared only within one quad or
 * polygon, through an index (16-bit when it can be): a quad keeps four corners instead of six.
 *
 * With `shareCorners`, the geometry has no normals and every corner with the same position,
 * colour, glow and hours is stored once, across faces. Its material must take each face's
 * normal from the screen instead (`flatShaded` in lighting.ts).
 */
export class MeshBuilder {
  private readonly shared: boolean;
  /** Open-addressed vertex index plus one per slot, 0 = empty; only with `shareCorners`. */
  private slots = new Int32Array(0);
  private capacity = 2 * 1024;
  private count = 0;
  private positions = new Float32Array(this.capacity * 3);
  /** Positions as raw bits, compared and hashed exactly. */
  private bits = new Uint32Array(this.positions.buffer);
  private normals = new Int8Array(this.capacity * 4);
  private colors = new Uint16Array(this.capacity * 4);
  private glows = new Uint8Array(this.capacity * 4);
  private hours = new Uint16Array(this.capacity * 2);
  private indices = new Uint32Array(3 * 1024);
  private length = 0;
  private glowing = false;
  private scheduled = false;
  // The face being written: packed normal, colour, glow and light hours.
  private qx = 0;
  private qy = 0;
  private qz = 0;
  private cr = 0;
  private cg = 0;
  private cb = 0;
  private er = 0;
  private eg = 0;
  private eb = 0;
  private h0 = 0;
  private h1 = 0;
  /** Emission colour for subsequently added faces; black means unlit. */
  readonly glow = new THREE.Color(0x000000);
  /** Daily on/off hours for subsequently added faces; zero duration stays dark. */
  lightSchedule: [number, number] = [0, 0];

  constructor({ shareCorners = false } = {}) {
    this.shared = shareCorners;
    if (shareCorners) this.slots = new Int32Array(2 * this.capacity);
  }

  get vertexCount(): number {
    return this.count;
  }

  /** Index entries so far, three per triangle; geometry groups count in these. */
  get indexCount(): number {
    return this.length;
  }

  private grow() {
    this.capacity *= 2;
    const grown = <T extends Float32Array | Int8Array | Uint8Array | Uint16Array>(
      old: T,
      per: number,
    ) => {
      const next = new (old.constructor as new (n: number) => T)(this.capacity * per);
      next.set(old);
      return next;
    };
    this.positions = grown(this.positions, 3);
    this.bits = new Uint32Array(this.positions.buffer);
    this.normals = grown(this.normals, 4);
    this.colors = grown(this.colors, 4);
    this.glows = grown(this.glows, 4);
    this.hours = grown(this.hours, 2);
  }

  // Scalars only below: this runs for every triangle of every rebuilt tile, so temporary arrays
  // here were the page's largest source of garbage while the city grows.

  /**
   * Packs the normal of triangle `a b c` for the face being written. Returns 0 if the triangle
   * is degenerate, -1 if its winding must flip so the normal points along `dir`, else 1.
   */
  private facing(a: V3, b: V3, c: V3, dir?: V3): number {
    const ux = b[0] - a[0];
    const uy = b[1] - a[1];
    const uz = b[2] - a[2];
    const vx = c[0] - a[0];
    const vy = c[1] - a[1];
    const vz = c[2] - a[2];
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    const len = Math.hypot(nx, ny, nz);
    if (len < 1e-9) return 0;
    let side = 1;
    if (dir && nx * dir[0] + ny * dir[1] + nz * dir[2] < 0) {
      side = -1;
      nx = -nx;
      ny = -ny;
      nz = -nz;
    }
    const scale = 127 / len;
    this.qx = Math.round(nx * scale);
    this.qy = Math.round(ny * scale);
    this.qz = Math.round(nz * scale);
    return side;
  }

  /** Sets the colour, glow and light hours of the face being written. */
  private paint(col: THREE.Color) {
    const on = this.lightSchedule[0];
    const off = this.lightSchedule[1];
    this.er = unit(this.glow.r, 255);
    this.eg = unit(this.glow.g, 255);
    this.eb = unit(this.glow.b, 255);
    this.glowing ||= this.er + this.eg + this.eb > 0;
    this.scheduled ||= on !== 0 || off !== 0;
    this.cr = unit(col.r, 65535);
    this.cg = unit(col.g, 65535);
    this.cb = unit(col.b, 65535);
    this.h0 = THREE.DataUtils.toHalfFloat(on);
    this.h1 = THREE.DataUtils.toHalfFloat(off);
  }

  /**
   * Appends a vertex at `p` with the face being written and returns its index, or the index of
   * an equal corner already stored when sharing corners.
   */
  private vertex(p: V3): number {
    if (this.count === this.capacity) this.grow();
    const i = this.count;
    const { positions, normals, colors, glows, hours } = this;
    positions[i * 3] = p[0];
    positions[i * 3 + 1] = p[1];
    positions[i * 3 + 2] = p[2];
    normals[i * 4] = this.qx;
    normals[i * 4 + 1] = this.qy;
    normals[i * 4 + 2] = this.qz;
    colors[i * 4] = this.cr;
    colors[i * 4 + 1] = this.cg;
    colors[i * 4 + 2] = this.cb;
    glows[i * 4] = this.er;
    glows[i * 4 + 1] = this.eg;
    glows[i * 4 + 2] = this.eb;
    hours[i * 2] = this.h0;
    hours[i * 2 + 1] = this.h1;
    if (this.shared) {
      const j = this.find(i);
      if (j !== i) return j;
    }
    return this.count++;
  }

  /** The stored corner equal to just-written corner `i`, else `i`, entered in the table. */
  private find(i: number): number {
    if (2 * (i + 1) > this.slots.length) this.rehash(this.slots.length * 2);
    const { bits, colors, glows, hours, slots } = this;
    const mask = slots.length - 1;
    let s = this.hash(i) & mask;
    for (; slots[s] !== 0; s = (s + 1) & mask) {
      const j = slots[s] - 1;
      if (
        bits[j * 3] === bits[i * 3] &&
        bits[j * 3 + 1] === bits[i * 3 + 1] &&
        bits[j * 3 + 2] === bits[i * 3 + 2] &&
        colors[j * 4] === colors[i * 4] &&
        colors[j * 4 + 1] === colors[i * 4 + 1] &&
        colors[j * 4 + 2] === colors[i * 4 + 2] &&
        glows[j * 4] === glows[i * 4] &&
        glows[j * 4 + 1] === glows[i * 4 + 1] &&
        glows[j * 4 + 2] === glows[i * 4 + 2] &&
        hours[j * 2] === hours[i * 2] &&
        hours[j * 2 + 1] === hours[i * 2 + 1]
      )
        return j;
    }
    slots[s] = i + 1;
    return i;
  }

  /** Table hash of stored corner `i`'s position; corners there differ only in paint. */
  private hash(i: number): number {
    const { bits } = this;
    let h = Math.imul(bits[i * 3] ^ (bits[i * 3 + 1] >>> 7), 0x9e3779b1);
    h = Math.imul(h ^ (h >>> 15) ^ bits[i * 3 + 2], 0x85ebca6b);
    h = Math.imul(h ^ (h >>> 13) ^ bits[i * 3 + 1], 0xc2b2ae35);
    return (h ^ (h >>> 16)) >>> 0;
  }

  /** Rebuilds the corner table with `size` slots, a power of two. */
  private rehash(size: number) {
    const slots = new Int32Array(size);
    const mask = size - 1;
    for (let j = 0; j < this.count; j++) {
      let s = this.hash(j) & mask;
      while (slots[s] !== 0) s = (s + 1) & mask;
      slots[s] = j + 1;
    }
    this.slots = slots;
  }

  /** Appends triangle `i j k`, wound `i k j` when `side` is negative. */
  private triangle(i: number, j: number, k: number, side: number) {
    if (this.length + 3 > this.indices.length) {
      const next = new Uint32Array(this.indices.length * 2);
      next.set(this.indices);
      this.indices = next;
    }
    this.indices[this.length++] = i;
    this.indices[this.length++] = side < 0 ? k : j;
    this.indices[this.length++] = side < 0 ? j : k;
  }

  /** One triangle. If `dir` is given, the winding is flipped so its normal points along `dir`. */
  tri(a: V3, b: V3, c: V3, col: THREE.Color, dir?: V3) {
    const side = this.facing(a, b, c, dir);
    if (side === 0) return;
    this.paint(col);
    this.triangle(this.vertex(a), this.vertex(b), this.vertex(c), side);
  }

  /** Triangles `a b c` and `a c d`, sharing their diagonal's vertices when they share a normal. */
  quad(a: V3, b: V3, c: V3, d: V3, col: THREE.Color, dir?: V3) {
    const first = this.facing(a, b, c, dir);
    const qx = this.qx;
    const qy = this.qy;
    const qz = this.qz;
    const second = this.facing(a, c, d, dir);
    if (first === 0 || first !== second || qx !== this.qx || qy !== this.qy || qz !== this.qz) {
      this.tri(a, b, c, col, dir);
      this.tri(a, c, d, col, dir);
      return;
    }
    this.paint(col);
    const i = this.vertex(a);
    const j = this.vertex(b);
    const k = this.vertex(c);
    const l = this.vertex(d);
    this.triangle(i, j, k, first);
    this.triangle(i, k, l, first);
  }

  /** An upward-facing polygon at height `y`, or at `y(p)` for sloped roof faces. */
  poly(pts: Pt[], y: number | ((p: Pt) => number), col: THREE.Color) {
    if (pts.length < 3) return;
    const shape = pts.map((p) => new THREE.Vector2(p[0], p[1]));
    const h = typeof y === 'number' ? () => y : y;
    const corners = pts.map((p): V3 => [p[0], h(p), p[1]]);
    // Each corner's latest vertex and its packed normal, reused by triangles in the same plane.
    const vertex = new Int32Array(pts.length).fill(-1);
    const normal = new Int32Array(pts.length);
    const at = (i: number, key: number) => {
      if (vertex[i] < 0 || normal[i] !== key) {
        vertex[i] = this.vertex(corners[i]);
        normal[i] = key;
      }
      return vertex[i];
    };
    for (const [a, b, c] of THREE.ShapeUtils.triangulateShape(shape, [])) {
      const side = this.facing(corners[a], corners[b], corners[c], UP);
      if (side === 0) continue;
      this.paint(col);
      const key = ((this.qx & 255) << 16) | ((this.qy & 255) << 8) | (this.qz & 255);
      this.triangle(at(a, key), at(b, key), at(c, key), side);
    }
  }

  /** A vertical wall quad from `p` to `q`, between heights `y0` and `y1`, facing `n`. */
  wall(p: Pt, q: Pt, y0: number, y1: number, col: THREE.Color, n: Pt) {
    this.quad([p[0], y0, p[1]], [q[0], y0, q[1]], [q[0], y1, q[1]], [p[0], y1, p[1]], col, [
      n[0],
      0,
      n[1],
    ]);
  }

  /** A box of size `sx` × `sy` × `sz` standing on `y0`, turned `rot` radians about +y. */
  box(c: Pt, y0: number, sx: number, sy: number, sz: number, rot: number, col: THREE.Color) {
    const [cs, sn] = [Math.cos(rot), Math.sin(rot)];
    const p = (x: number, z: number): Pt => [c[0] + x * cs + z * sn, c[1] - x * sn + z * cs];
    const ring = [p(-sx / 2, -sz / 2), p(sx / 2, -sz / 2), p(sx / 2, sz / 2), p(-sx / 2, sz / 2)];
    this.prism(ring, y0, y0 + sy, col, true);
  }

  /** An n-sided upright prism (a low-poly cylinder) or, with `top` radius 0, a cone. */
  cylinder(c: Pt, y0: number, r: number, h: number, sides: number, col: THREE.Color, top = r) {
    const ring = (radius: number): Pt[] =>
      Array.from({ length: sides }, (_, i) => {
        const a = (i / sides) * Math.PI * 2;
        return [c[0] + Math.cos(a) * radius, c[1] + Math.sin(a) * radius];
      });
    const [bottom, upper] = [ring(r), ring(top)];
    for (let i = 0; i < sides; i++) {
      const j = (i + 1) % sides;
      const out: V3 = [
        Math.cos(((i + 0.5) / sides) * Math.PI * 2),
        0,
        Math.sin(((i + 0.5) / sides) * Math.PI * 2),
      ];
      const [a, b] = [bottom[i], bottom[j]];
      const [d, e] = [upper[i], upper[j]];
      if (top > 0)
        this.quad(
          [a[0], y0, a[1]],
          [b[0], y0, b[1]],
          [e[0], y0 + h, e[1]],
          [d[0], y0 + h, d[1]],
          col,
          out,
        );
      else
        this.tri([a[0], y0, a[1]], [b[0], y0, b[1]], [c[0], y0 + h, c[1]], col, [
          out[0],
          r / h,
          out[2],
        ]);
    }
    if (top > 0) this.poly(upper, y0 + h, col);
  }

  /** Walls of `ring` from `y0` to `y1`, with an optional flat cap. */
  prism(ring: Pt[], y0: number, y1: number, col: THREE.Color, cap: boolean) {
    for (let i = 0; i < ring.length; i++) {
      const [p, q] = [ring[i], ring[(i + 1) % ring.length]];
      this.wall(p, q, y0, y1, col, [q[1] - p[1], -(q[0] - p[0])]);
    }
    if (cap) this.poly(ring, y1, col);
  }

  /**
   * A convex `shape` drawn in the x–y plane, given depth along z: `halfWidth(y)` either side of
   * `zc`. A width that shrinks with height bevels the top edges.
   */
  extrude(shape: Pt[], halfWidth: number | ((y: number) => number), zc: number, col: THREE.Color) {
    const hw = typeof halfWidth === 'number' ? () => halfWidth : halfWidth;
    const v = (p: Pt, s: number): V3 => [p[0], p[1], zc + s * hw(p[1])];
    // triangulateShape may reverse its input, so read points back from the array it indexed.
    const pts = shape.map((p) => new THREE.Vector2(p[0], p[1]));
    const tris = THREE.ShapeUtils.triangulateShape(pts, []);
    for (const s of [1, -1])
      for (const [a, b, c] of tris) {
        const [pa, pb, pc] = [a, b, c].map((i): Pt => [pts[i].x, pts[i].y]);
        this.tri(v(pa, s), v(pb, s), v(pc, s), col, [0, 0, s]);
      }
    const cx = shape.reduce((sum, p) => sum + p[0], 0) / shape.length;
    const cy = shape.reduce((sum, p) => sum + p[1], 0) / shape.length;
    for (let i = 0; i < shape.length; i++) {
      const [p, q] = [shape[i], shape[(i + 1) % shape.length]];
      const out: V3 = [(p[0] + q[0]) / 2 - cx, (p[1] + q[1]) / 2 - cy, 0];
      this.quad(v(p, 1), v(q, 1), v(q, -1), v(p, -1), col, out);
    }
  }

  geometry(): THREE.BufferGeometry {
    const n = this.count;
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(this.positions.slice(0, n * 3), 3));
    if (!this.shared) g.setAttribute('normal', packed(this.normals.slice(0, n * 4)));
    g.setAttribute('color', packed(this.colors.slice(0, n * 4)));
    if (this.glowing) g.setAttribute('nightEmission', packed(this.glows.slice(0, n * 4)));
    // Float16BufferAttribute copies its array, so a view avoids copying twice.
    if (this.scheduled)
      g.setAttribute(
        'lightSchedule',
        new THREE.Float16BufferAttribute(this.hours.subarray(0, n * 2), 2),
      );
    const index = this.indices.subarray(0, this.length);
    // Most tiles have few enough corners for 16-bit entries, which halve the index.
    g.setIndex(new THREE.BufferAttribute(n <= 65536 ? Uint16Array.from(index) : index.slice(), 1));
    g.computeBoundingSphere();
    return g;
  }
}
