// Conservative placement: omit details rather than cross a property, wall or road.
import { along, ccw, clip, type Edge, inside, type Pt } from '../building-kit/polygon.ts';
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

function intersects(a: Pt[], b: Pt[]) {
  return a.some((p, i) =>
    b.some((q, j) => crosses(p, a[(i + 1) % a.length], q, b[(j + 1) % b.length])),
  );
}

/** All edges, not only corners, must fit; `occupied` includes walls and prior details. */
export function fits(shape: Pt[], lot: Pt[], occupied: Pt[][], roads: Clearance): boolean {
  if (!shape.every((p) => inside(p, lot)) || intersects(shape, lot)) return false;
  for (const ring of occupied) {
    if (
      shape.some((p) => inside(p, ring)) ||
      ring.some((p) => inside(p, shape)) ||
      intersects(shape, ring)
    )
      return false;
  }
  // roadGap is a distance field (1-Lipschitz). Quarter-metre edge samples plus
  // half that spacing keep every edge at least 0.05 m off asphalt, even diagonally.
  for (let i = 0; i < shape.length; i++) {
    const [a, b] = [shape[i], shape[(i + 1) % shape.length]];
    const steps = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) * 4));
    for (let j = 0; j <= steps; j++) {
      const t = j / steps;
      if (roads.roadGap(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t) < 0.175) return false;
    }
  }
  return true;
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
