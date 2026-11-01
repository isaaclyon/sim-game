// Building footprints and heights come from the sim. This module only triangulates and shades.
import * as THREE from 'three';

/** Wall colors by zone; density is also visible in the sim-owned height. */
const WALLS = [0, 0xd8c6a5, 0xc3d1be, 0xa5c9d5, 0x87adbf, 0xc8b68d, 0xb8a282];
const ROOFS = [0, 0xa96e52, 0x718777, 0x537f90, 0x476979, 0x8c8271, 0x736959];

/** Reads [zone, height, n, x0, y0, …] per building. Sim (x,y) becomes local (x,z). */
export function buildBuildingGeometry(data: Float32Array): THREE.BufferGeometry {
  const positions: number[] = [];
  const colors: number[] = [];
  const vertex = (p: THREE.Vector2, height: number, color: THREE.Color) => {
    positions.push(p.x, height, p.y);
    colors.push(color.r, color.g, color.b);
  };
  let offset = 0;
  while (offset < data.length) {
    const zone = data[offset++];
    const height = data[offset++];
    const count = data[offset++];
    const ring: THREE.Vector2[] = [];
    for (let i = 0; i < count; i++) {
      ring.push(new THREE.Vector2(data[offset++], data[offset++]));
    }
    const roof = new THREE.Color(ROOFS[zone]);
    const wall = new THREE.Color(WALLS[zone]);
    for (const [a, b, c] of THREE.ShapeUtils.triangulateShape(ring, [])) {
      // XY triangulation winds upwards only after reversing it into the XZ plane.
      vertex(ring[c], height, roof);
      vertex(ring[b], height, roof);
      vertex(ring[a], height, roof);
    }
    for (let i = 0; i < ring.length; i++) {
      const a = ring[i];
      const b = ring[(i + 1) % ring.length];
      vertex(a, 0, wall);
      vertex(a, height, wall);
      vertex(b, height, wall);
      vertex(a, 0, wall);
      vertex(b, height, wall);
      vertex(b, 0, wall);
    }
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}
