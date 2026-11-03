// Conservative placement: omit details rather than cross a property, wall or road.
import { inside, type Pt } from '../building-kit/polygon.ts';
import type { Clearance } from '../scenery/clearance.ts';

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
