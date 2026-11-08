// Flat-shaded, vertex-coloured triangle soup in scene-local space (x, height, z).
import * as THREE from 'three';
import type { Pt } from './polygon.ts';

export type V3 = [number, number, number];

/** Unit float in [0, 1] to a normalized integer up to `max`; three.js's own conversion wraps. */
const unit = (v: number, max: number) => Math.round(Math.min(Math.max(v, 0), 1) * max);

/** Three values per vertex, padded to four so GPUs read them without converting the buffer. */
function packed(array: Int8Array | Uint8Array | Uint16Array): THREE.InterleavedBufferAttribute {
  return new THREE.InterleavedBufferAttribute(new THREE.InterleavedBuffer(array, 4), 3, 0, true);
}

/**
 * Vertices are stored compactly, since a city holds tens of millions of them twice (here and on
 * the GPU): float positions, byte normals and glow, 16-bit colours (bytes would lose dark
 * palette colours, which are linear here) and half-float light hours. Glow and hours are left
 * out when no face has any; the night shader then reads zeros.
 */
export class MeshBuilder {
  private capacity = 3 * 1024;
  private count = 0;
  private positions = new Float32Array(this.capacity * 3);
  private normals = new Int8Array(this.capacity * 4);
  private colors = new Uint16Array(this.capacity * 4);
  private glows = new Uint8Array(this.capacity * 4);
  private hours = new Uint16Array(this.capacity * 2);
  private glowing = false;
  private scheduled = false;
  /** Emission colour for subsequently added faces; black means unlit. */
  readonly glow = new THREE.Color(0x000000);
  /** Daily on/off hours for subsequently added faces; zero duration stays dark. */
  lightSchedule: [number, number] = [0, 0];

  get vertexCount(): number {
    return this.count;
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
    this.normals = grown(this.normals, 4);
    this.colors = grown(this.colors, 4);
    this.glows = grown(this.glows, 4);
    this.hours = grown(this.hours, 2);
  }

  /** One triangle. If `dir` is given, the winding is flipped so its normal points along `dir`. */
  // Scalars only: this runs for every triangle of every rebuilt tile, so temporary arrays here
  // were the page's largest source of garbage while the city grows.
  tri(a: V3, b: V3, c: V3, col: THREE.Color, dir?: V3) {
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
    if (len < 1e-9) return;
    let second = b;
    let third = c;
    if (dir && nx * dir[0] + ny * dir[1] + nz * dir[2] < 0) {
      second = c;
      third = b;
      nx = -nx;
      ny = -ny;
      nz = -nz;
    }
    if (this.count + 3 > this.capacity) this.grow();
    const on = this.lightSchedule[0];
    const off = this.lightSchedule[1];
    const er = unit(this.glow.r, 255);
    const eg = unit(this.glow.g, 255);
    const eb = unit(this.glow.b, 255);
    this.glowing ||= er + eg + eb > 0;
    this.scheduled ||= on !== 0 || off !== 0;
    const scale = 127 / len;
    const qx = Math.round(nx * scale);
    const qy = Math.round(ny * scale);
    const qz = Math.round(nz * scale);
    const cr = unit(col.r, 65535);
    const cg = unit(col.g, 65535);
    const cb = unit(col.b, 65535);
    const h0 = THREE.DataUtils.toHalfFloat(on);
    const h1 = THREE.DataUtils.toHalfFloat(off);
    const { positions, normals, colors, glows, hours } = this;
    for (let v = 0; v < 3; v++) {
      const p = v === 0 ? a : v === 1 ? second : third;
      const i = this.count++;
      positions[i * 3] = p[0];
      positions[i * 3 + 1] = p[1];
      positions[i * 3 + 2] = p[2];
      normals[i * 4] = qx;
      normals[i * 4 + 1] = qy;
      normals[i * 4 + 2] = qz;
      colors[i * 4] = cr;
      colors[i * 4 + 1] = cg;
      colors[i * 4 + 2] = cb;
      glows[i * 4] = er;
      glows[i * 4 + 1] = eg;
      glows[i * 4 + 2] = eb;
      hours[i * 2] = h0;
      hours[i * 2 + 1] = h1;
    }
  }

  quad(a: V3, b: V3, c: V3, d: V3, col: THREE.Color, dir?: V3) {
    this.tri(a, b, c, col, dir);
    this.tri(a, c, d, col, dir);
  }

  /** An upward-facing polygon at height `y`, or at `y(p)` for sloped roof faces. */
  poly(pts: Pt[], y: number | ((p: Pt) => number), col: THREE.Color) {
    if (pts.length < 3) return;
    const shape = pts.map((p) => new THREE.Vector2(p[0], p[1]));
    const h = typeof y === 'number' ? () => y : y;
    for (const t of THREE.ShapeUtils.triangulateShape(shape, [])) {
      const [a, b, c] = t.map((i): V3 => [pts[i][0], h(pts[i]), pts[i][1]]);
      this.tri(a, b, c, col, [0, 1, 0]);
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
    g.setAttribute('normal', packed(this.normals.slice(0, n * 4)));
    g.setAttribute('color', packed(this.colors.slice(0, n * 4)));
    if (this.glowing) g.setAttribute('nightEmission', packed(this.glows.slice(0, n * 4)));
    // Float16BufferAttribute copies its array, so a view avoids copying twice.
    if (this.scheduled)
      g.setAttribute(
        'lightSchedule',
        new THREE.Float16BufferAttribute(this.hours.subarray(0, n * 2), 2),
      );
    g.computeBoundingSphere();
    return g;
  }
}
