import * as THREE from 'three';

// Two lane centres at ±2 m, 2 m wide cars, plus 1 m of shoulder on each side.
const HALF_WIDTH = 4;
const PAD_SIDES = 12;

/** Flat asphalt ribbons and shared-endpoint pads from `[ax, ay, bx, by, speedLimit]`.
 * This is display geometry only; the sim remains responsible for joining roads.
 */
export function buildRoadGeometry(segments: Float32Array): THREE.BufferGeometry {
  const positions: number[] = [];
  const endpoints = new Map<string, { x: number; z: number; count: number }>();
  function endpoint(x: number, z: number) {
    const key = `${x},${z}`;
    const point = endpoints.get(key);
    if (point) point.count++;
    else endpoints.set(key, { x, z, count: 1 });
  }

  for (let i = 0; i < segments.length; i += 5) {
    const [ax, az, bx, bz] = segments.subarray(i, i + 4);
    const dx = bx - ax;
    const dz = bz - az;
    const length = Math.hypot(dx, dz);
    if (length === 0) continue;
    const nx = (-dz / length) * HALF_WIDTH;
    const nz = (dx / length) * HALF_WIDTH;
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
    endpoint(ax, az);
    endpoint(bx, bz);
  }

  // Circumscribe the road half-width so even oblique joins have no wedge-shaped gaps.
  // Pads and ribbons share one unlit material and plane; overlaps have identical color.
  const radius = HALF_WIDTH / Math.cos(Math.PI / PAD_SIDES);
  for (const { x, z, count } of endpoints.values()) {
    if (count < 2) continue;
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
  return geometry;
}
