// Flat-shaded, vertex-coloured triangle soup in scene-local space (x, height, z).
import * as THREE from 'three';
import type { Pt } from './polygon.ts';

export type V3 = [number, number, number];

export class MeshBuilder {
  /** Interleaved per vertex: position (3), normal (3), colour (3). Grows by doubling. */
  private data = new Float32Array(9 * 3 * 1024);
  private size = 0;

  get vertexCount(): number {
    return this.size / 9;
  }

  /** One triangle. If `dir` is given, the winding is flipped so its normal points along `dir`. */
  tri(a: V3, b: V3, c: V3, col: THREE.Color, dir?: V3) {
    const [ux, uy, uz] = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const [vx, vy, vz] = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let [nx, ny, nz] = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx];
    const len = Math.hypot(nx, ny, nz);
    if (len < 1e-9) return;
    if (dir && nx * dir[0] + ny * dir[1] + nz * dir[2] < 0) {
      [b, c] = [c, b];
      [nx, ny, nz] = [-nx, -ny, -nz];
    }
    if (this.size + 27 > this.data.length) {
      const grown = new Float32Array(this.data.length * 2);
      grown.set(this.data);
      this.data = grown;
    }
    const d = this.data;
    for (const p of [a, b, c]) {
      const o = this.size;
      d[o] = p[0];
      d[o + 1] = p[1];
      d[o + 2] = p[2];
      d[o + 3] = nx / len;
      d[o + 4] = ny / len;
      d[o + 5] = nz / len;
      d[o + 6] = col.r;
      d[o + 7] = col.g;
      d[o + 8] = col.b;
      this.size += 9;
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

  geometry(): THREE.BufferGeometry {
    const buffer = new THREE.InterleavedBuffer(this.data.slice(0, this.size), 9);
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.InterleavedBufferAttribute(buffer, 3, 0));
    g.setAttribute('normal', new THREE.InterleavedBufferAttribute(buffer, 3, 3));
    g.setAttribute('color', new THREE.InterleavedBufferAttribute(buffer, 3, 6));
    g.computeBoundingSphere();
    return g;
  }
}
