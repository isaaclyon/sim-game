import * as THREE from 'three';
import type { RoadClass } from './protocol';

// Two lane centres at ±2 m, 2 m wide cars, plus 1 m of shoulder on each side.
const HALF_WIDTH = 4;
// Main roads have the same two lanes but wide shoulders, so they read as main roads.
const MAIN_HALF_WIDTH = 6;
const PAD_SIDES = 12;
// `RoadClass.Main`, inlined so this module loads in Node tests without a bundler.
const MAIN: RoadClass = 1;

/** Flat asphalt ribbons and shared-endpoint pads from `[ax, ay, bx, by, class]`.
 * This is display geometry only; the sim remains responsible for joining roads.
 * `extra` widens every road on both sides (sidewalks); `colors`, indexed by class, adds a
 * vertex colour attribute. Pads take the colour of their widest road.
 */
export function buildRoadGeometry(
  segments: Float32Array,
  extra = 0,
  colors?: THREE.Color[],
): THREE.BufferGeometry {
  const positions: number[] = [];
  const rgb: number[] = [];
  const paint = (cls: number, vertices: number) => {
    const c = colors?.[cls] ?? colors?.[0];
    if (c) for (let i = 0; i < vertices; i++) rgb.push(c.r, c.g, c.b);
  };
  const endpoints = new Map<
    string,
    { x: number; z: number; count: number; half: number; cls: number }
  >();
  function endpoint(x: number, z: number, half: number, cls: number) {
    const key = `${x},${z}`;
    const point = endpoints.get(key);
    if (point) {
      point.count++;
      if (half > point.half) Object.assign(point, { half, cls });
    } else endpoints.set(key, { x, z, count: 1, half, cls });
  }

  for (let i = 0; i < segments.length; i += 5) {
    const [ax, az, bx, bz, cls] = segments.subarray(i, i + 5);
    const dx = bx - ax;
    const dz = bz - az;
    const length = Math.hypot(dx, dz);
    if (length === 0) continue;
    const half = (cls === MAIN ? MAIN_HALF_WIDTH : HALF_WIDTH) + extra;
    const nx = (-dz / length) * half;
    const nz = (dx / length) * half;
    // Counterclockwise from above: front faces point toward world +y.
    positions.push(
      ax + nx,
      0,
      az + nz,
      bx + nx,
      0,
      bz + nz,
      ax - nx,
      0,
      az - nz,
      ax - nx,
      0,
      az - nz,
      bx + nx,
      0,
      bz + nz,
      bx - nx,
      0,
      bz - nz,
    );
    paint(cls, 6);
    endpoint(ax, az, half, cls);
    endpoint(bx, bz, half, cls);
  }

  // Circumscribe the widest road's half-width so even oblique joins have no wedge-shaped gaps.
  // Pads and ribbons share one unlit material and plane; overlaps have identical color.
  for (const { x, z, count, half, cls } of endpoints.values()) {
    if (count < 2) continue;
    paint(cls, PAD_SIDES * 3);
    const radius = half / Math.cos(Math.PI / PAD_SIDES);
    for (let i = 0; i < PAD_SIDES; i++) {
      const a = (i / PAD_SIDES) * Math.PI * 2;
      const b = ((i + 1) / PAD_SIDES) * Math.PI * 2;
      positions.push(
        x,
        0,
        z,
        x + Math.cos(b) * radius,
        0,
        z + Math.sin(b) * radius,
        x + Math.cos(a) * radius,
        0,
        z + Math.sin(a) * radius,
      );
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  if (colors) geometry.setAttribute('color', new THREE.Float32BufferAttribute(rgb, 3));
  return geometry;
}
