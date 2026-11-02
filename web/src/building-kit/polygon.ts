// 2D helpers for footprints in scene-local (x, z), which is sim (x, y).

export type Pt = [number, number];

/** Signed area; positive for the sim's counter-clockwise rings. */
export function area(p: Pt[]): number {
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const q = p[(i + 1) % p.length];
    a += p[i][0] * q[1] - q[0] * p[i][1];
  }
  return a / 2;
}

export function ccw(p: Pt[]): Pt[] {
  return area(p) > 0 ? p : p.slice().reverse();
}

export function norm(v: Pt): Pt {
  const l = Math.hypot(v[0], v[1]) || 1;
  return [v[0] / l, v[1] / l];
}

export function inside(pt: Pt, p: Pt[]): boolean {
  let c = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    if (
      p[i][1] > pt[1] !== p[j][1] > pt[1] &&
      pt[0] < ((p[j][0] - p[i][0]) * (pt[1] - p[i][1])) / (p[j][1] - p[i][1]) + p[i][0]
    )
      c = !c;
  }
  return c;
}

export function convex(p: Pt[]): boolean {
  for (let i = 0; i < p.length; i++) {
    const [a, b, c] = [p[i], p[(i + 1) % p.length], p[(i + 2) % p.length]];
    if ((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) < -1e-6) return false;
  }
  return true;
}

/** Moves each edge of a counter-clockwise ring outward by `d` (inward if negative). Only
 * reliable on convex rings; callers guard concave footprints. */
export function offset(p: Pt[], d: number): Pt[] {
  const n = p.length;
  return p.map((b, i) => {
    const a = p[(i - 1 + n) % n];
    const c = p[(i + 1) % n];
    const e1 = norm([b[0] - a[0], b[1] - a[1]]);
    const e2 = norm([c[0] - b[0], c[1] - b[1]]);
    const m: Pt = [e1[1] + e2[1], -e1[0] - e2[0]];
    const k = d / Math.max(1 + e1[0] * e2[0] + e1[1] * e2[1], 0.3);
    return [b[0] + m[0] * k, b[1] + m[1] * k];
  });
}

/** Keeps the part of `poly` where `g >= 0`, for an affine `g` (one half-plane). */
export function clip(poly: Pt[], g: (p: Pt) => number): Pt[] {
  const out: Pt[] = [];
  for (let i = 0; i < poly.length; i++) {
    const p = poly[i];
    const q = poly[(i + 1) % poly.length];
    const [gp, gq] = [g(p), g(q)];
    if (gp >= 0) out.push(p);
    if (gp >= 0 !== gq >= 0) {
      const t = gp / (gp - gq);
      out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
    }
  }
  return out;
}

/** Minimum-area box aligned to a footprint edge: centre, long axis `u`, half sizes `L >= W`. */
export interface Box {
  c: Pt;
  u: Pt;
  v: Pt;
  L: number;
  W: number;
}

export function obb(p: Pt[]): Box {
  let best: (Box & { area: number }) | null = null;
  for (let i = 0; i < p.length; i++) {
    const q = p[(i + 1) % p.length];
    const u = norm([q[0] - p[i][0], q[1] - p[i][1]]);
    const v: Pt = [-u[1], u[0]];
    let [a0, a1, b0, b1] = [Infinity, -Infinity, Infinity, -Infinity];
    for (const r of p) {
      const a = r[0] * u[0] + r[1] * u[1];
      const b = r[0] * v[0] + r[1] * v[1];
      [a0, a1, b0, b1] = [Math.min(a0, a), Math.max(a1, a), Math.min(b0, b), Math.max(b1, b)];
    }
    const boxArea = (a1 - a0) * (b1 - b0);
    if (!best || boxArea < best.area - 1e-6) {
      const [ca, cb] = [(a0 + a1) / 2, (b0 + b1) / 2];
      const c: Pt = [u[0] * ca + v[0] * cb, u[1] * ca + v[1] * cb];
      best = { area: boxArea, c, u, v, L: (a1 - a0) / 2, W: (b1 - b0) / 2 };
    }
  }
  if (!best) throw new Error('empty footprint');
  const { c, u, v, L, W } = best;
  return W > L ? { c, u: v, v: [-u[0], -u[1]], L: W, W: L } : { c, u, v, L, W };
}

/** Point at `a` along the box's long axis and `b` across it, from its centre. */
export function at(box: Box, a: number, b: number): Pt {
  return [box.c[0] + box.u[0] * a + box.v[0] * b, box.c[1] + box.u[1] * a + box.v[1] * b];
}

export interface Edge {
  a: Pt;
  L: number;
  /** Unit direction along the edge. */
  u: Pt;
  /** Unit outward normal for counter-clockwise rings. */
  n: Pt;
}

export function edge(poly: Pt[], i: number): Edge {
  const a = poly[i % poly.length];
  const b = poly[(i + 1) % poly.length];
  const L = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
  const u: Pt = [(b[0] - a[0]) / L, (b[1] - a[1]) / L];
  return { a, L, u, n: [u[1], -u[0]] };
}

/** Point `s` metres along edge `e` and `out` metres in front of it. */
export function along(e: Edge, s: number, out = 0): Pt {
  return [e.a[0] + e.u[0] * s + e.n[0] * out, e.a[1] + e.u[1] * s + e.n[1] * out];
}
