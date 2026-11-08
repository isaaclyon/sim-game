// Conservative placement: omit details rather than cross a property, wall or road.
import {
  along,
  ccw,
  clip,
  type Edge,
  inside,
  type Pt,
  rayCrosses,
} from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';

/** A rectangle aligned to a building edge, with its centre `out` metres outside it. */
export function rectangle(e: Edge, s: number, out: number, width: number, depth: number): Pt[] {
  return ccw([
    along(e, s - width / 2, out - depth / 2),
    along(e, s + width / 2, out - depth / 2),
    along(e, s + width / 2, out + depth / 2),
    along(e, s - width / 2, out + depth / 2),
  ]);
}

function cross(a: Pt, b: Pt, c: Pt) {
  return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
}

function crosses(a: Pt, b: Pt, c: Pt, d: Pt) {
  return cross(a, b, c) * cross(a, b, d) < -1e-8 && cross(c, d, a) * cross(c, d, b) < -1e-8;
}

/** Whether `pq` meets any of `others`, touching or overlapping included. */
export function meetsAny(p: Pt, q: Pt, others: Side[]) {
  for (const [a, b] of others)
    if (cross(p, q, a) * cross(p, q, b) <= 1e-8 && cross(a, b, p) * cross(a, b, q) <= 1e-8)
      return true;
  return false;
}

/** A ring's side, from one point to the next. */
type Side = [Pt, Pt];

export const sides = (ring: Pt[]): Side[] => ring.map((p, i) => [p, ring[(i + 1) % ring.length]]);

/** Whether any side of `shape` crosses one of `others`. */
export function crossesAny(shape: Pt[], others: Side[]) {
  for (let i = 0; i < shape.length; i++) {
    const [p, q] = [shape[i], shape[i + 1 === shape.length ? 0 : i + 1]];
    for (const [a, b] of others) if (crosses(p, q, a, b)) return true;
  }
  return false;
}

/** Metres added around boxes, far more than float rounding could reach. */
const PAD = 1e-3;

/** `[x0, y0, x1, y1]` around `points`, padded. */
function box(points: Pt[]): number[] {
  const out = [Infinity, Infinity, -Infinity, -Infinity];
  for (const [x, y] of points) {
    out[0] = Math.min(out[0], x - PAD);
    out[1] = Math.min(out[1], y - PAD);
    out[2] = Math.max(out[2], x + PAD);
    out[3] = Math.max(out[3], y + PAD);
  }
  return out;
}

const apart = (a: number[], b: number[]) =>
  a[2] < b[0] || b[2] < a[0] || a[3] < b[1] || b[3] < a[1];

/** The sides and corners of `ring` that can matter to shapes inside the padded box `r`. */
function nearby(ring: Pt[], r: number[]) {
  const ray: Side[] = [];
  const near: Side[] = [];
  const points: Pt[] = [];
  for (let i = 0; i < ring.length; i++) {
    const [a, b] = [ring[i], ring[i + 1 === ring.length ? 0 : i + 1]];
    if (a[0] >= r[0] && a[0] <= r[2] && a[1] >= r[1] && a[1] <= r[3]) points.push(a);
    const [x0, x1] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
    const [y0, y1] = [Math.min(a[1], b[1]), Math.max(a[1], b[1])];
    // A ray toward +x from a point in the box only passes sides level with it, not behind it.
    if (y1 < r[1] || y0 > r[3] || x1 < r[0]) continue;
    ray.push([a, b]);
    if (x0 - PAD <= r[2] && x1 + PAD >= r[0] && y0 - PAD <= r[3] && y1 + PAD >= r[1])
      near.push([a, b]);
  }
  return { ray, near, points };
}

/** `inside`, counting only the sides a ray from `p` may pass. */
function within(p: Pt, ray: Side[]) {
  let c = false;
  for (const [a, b] of ray) if (rayCrosses(p, a, b)) c = !c;
  return c;
}

/**
 * `fits` for shapes inside `region`'s bounding box, such as the pieces a boundary strip is cut
 * into. Sides and corners of the lot and occupied rings that cannot reach the box are set aside
 * once, which never changes an answer: a point outside a ring's box is outside the ring, sides
 * whose boxes are apart cannot cross, and a ray toward +x only passes sides level with it and
 * not behind it.
 */
export function fitter(
  region: Pt[],
  lot: Pt[],
  occupied: Pt[][],
  roads: Clearance,
): (shape: Pt[]) => boolean {
  const r = box(region);
  const property = nearby(lot, r);
  const rings = occupied.filter((ring) => !apart(box(ring), r)).map((ring) => nearby(ring, r));
  const roadGap = roads.gapsNear(r, ROAD_GAP);
  return (shape) => {
    for (const p of shape) if (!within(p, property.ray)) return false;
    if (crossesAny(shape, property.near)) return false;
    for (const ring of rings) {
      for (const p of shape) if (within(p, ring.ray)) return false;
      for (const p of ring.points) if (inside(p, shape)) return false;
      if (crossesAny(shape, ring.near)) return false;
    }
    // roadGap is a distance field (1-Lipschitz). Quarter-metre edge samples plus
    // half that spacing keep every edge at least 0.05 m off asphalt, even diagonally.
    if (!roadGap) return true;
    for (let i = 0; i < shape.length; i++) {
      const [a, b] = [shape[i], shape[(i + 1) % shape.length]];
      const steps = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) * 4));
      for (let j = 0; j <= steps; j++) {
        const t = j / steps;
        if (roadGap(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t) < ROAD_GAP) return false;
      }
    }
    return true;
  };
}

/** Closest a shape's edges may sample to asphalt, in metres. */
export const ROAD_GAP = 0.175;

/** All edges, not only corners, must fit; `occupied` includes walls and prior details. */
export function fits(shape: Pt[], lot: Pt[], occupied: Pt[][], roads: Clearance): boolean {
  return fitter(shape, lot, occupied, roads)(shape);
}

/** First property exit along an outward ray, including concave lot edges. */
export function reach(lot: Pt[], e: Edge, s: number, start: number): number {
  const local = (p: Pt): Pt => [
    (p[0] - e.a[0]) * e.u[0] + (p[1] - e.a[1]) * e.u[1],
    (p[0] - e.a[0]) * e.n[0] + (p[1] - e.a[1]) * e.n[1],
  ];
  let end = Infinity;
  for (let i = 0; i < lot.length; i++) {
    const [a, b] = [local(lot[i]), local(lot[(i + 1) % lot.length])];
    if (Math.abs(b[0] - a[0]) < 1e-8) continue;
    const t = (s - a[0]) / (b[0] - a[0]);
    const out = a[1] + t * (b[1] - a[1]);
    if (t >= 0 && t <= 1 && out > start) end = Math.min(end, out);
  }
  return end;
}

/** Connect to the nearest frontage, not an arbitrary-length strip through the property. */
export function access(
  lot: Pt[],
  e: Edge,
  s: number,
  width: number,
  start: number,
  occupied: Pt[][],
  roads: Clearance,
): Pt[] | null {
  const samples = [s - width / 2, s, s + width / 2];
  let end = Math.min(...samples.map((at) => reach(lot, e, at, start))) - 0.22;
  if (!Number.isFinite(end)) return null;
  // Main roads can extend inside the fixed lot kerb. Stop paving just before their asphalt.
  while (end > start && samples.some((at) => roads.roadGap(...along(e, at, end)) < 0.2))
    end -= 0.25;
  if (end - start < 0.25) return null;
  const shape = rectangle(e, s, (start + end) / 2, width, end - start);
  return fits(shape, lot, occupied, roads) ? shape : null;
}

/**
 * The part of the lot directly in front of edge `e`, out to the first asphalt. Unlike
 * `access`, it follows tapered and irregular lot sides instead of needing a rectangle.
 */
export function frontage(lot: Pt[], e: Edge, roads: Clearance): Pt[] | null {
  // Roads draw over lot ground, so the frontage may run right up to the asphalt edge.
  let end = 60;
  for (const s of [0, e.L / 2, e.L]) {
    // roadGap is a distance, so each step can safely jump that far (sphere tracing).
    let out = 0.02;
    for (let gap = roads.roadGap(...along(e, s, out)); out < end && gap > 0.02; ) {
      out += gap;
      gap = roads.roadGap(...along(e, s, out));
    }
    end = Math.min(end, out);
  }
  const s = (p: Pt) => (p[0] - e.a[0]) * e.u[0] + (p[1] - e.a[1]) * e.u[1];
  const out = (p: Pt) => (p[0] - e.a[0]) * e.n[0] + (p[1] - e.a[1]) * e.n[1];
  let shape = clip(lot, (p) => out(p) - 0.02);
  shape = clip(shape, (p) => end - out(p));
  shape = clip(shape, s);
  shape = clip(shape, (p) => e.L - s(p));
  return shape.length >= 3 ? shape : null;
}
