// Flat, vertex-coloured ground meshes shared by the data overlays (ADR 0057): road ribbons
// and filled lot outlines. Display only.
import * as THREE from 'three';

export const flat = (opacity: number) =>
  new THREE.MeshBasicMaterial({
    vertexColors: true,
    transparent: true,
    opacity,
    depthWrite: false,
    side: THREE.DoubleSide,
    fog: false,
  });

/** An empty, hidden mesh at height `y`; `upload` fills it. */
export function mesh(material: THREE.Material, renderOrder: number, y: number): THREE.Mesh {
  const m = new THREE.Mesh(new THREE.BufferGeometry(), material);
  m.renderOrder = renderOrder;
  m.position.y = y;
  m.frustumCulled = false;
  m.visible = false;
  return m;
}

/** Triangles being collected for one mesh. */
export class Triangles {
  readonly positions: number[] = [];
  readonly colors: number[] = [];

  /** A ribbon `half` wide each side of the segment, and `half` past each end so bends and
   * junctions have no gaps. */
  segment(ax: number, ay: number, bx: number, by: number, half: number, c: THREE.Color) {
    const length = Math.hypot(bx - ax, by - ay);
    if (length === 0) return;
    const [ux, uy] = [((bx - ax) / length) * half, ((by - ay) / length) * half];
    const [p, q] = [
      [ax - ux, ay - uy],
      [bx + ux, by + uy],
    ];
    const corners = [
      [p[0] - uy, p[1] + ux],
      [q[0] - uy, q[1] + ux],
      [q[0] + uy, q[1] - ux],
      [p[0] + uy, p[1] - ux],
    ];
    for (const k of [0, 1, 2, 0, 2, 3]) this.point(corners[k][0], corners[k][1], c);
  }

  /** A filled outline of `n` points, `[x, y]` each, starting at `data[at]`. */
  ring(data: ArrayLike<number>, at: number, n: number, c: THREE.Color) {
    const ring: THREE.Vector2[] = [];
    for (let k = 0; k < n; k++)
      ring.push(new THREE.Vector2(data[at + 2 * k], data[at + 1 + 2 * k]));
    // triangulateShape may reverse the ring; its indices point into the array as left.
    for (const tri of THREE.ShapeUtils.triangulateShape(ring, []))
      for (const k of tri) this.point(ring[k].x, ring[k].y, c);
  }

  private point(x: number, y: number, c: THREE.Color) {
    this.positions.push(x, 0, y);
    this.colors.push(c.r, c.g, c.b);
  }

  /** Replaces `m`'s geometry with these triangles. */
  upload(m: THREE.Mesh) {
    m.geometry.dispose();
    m.geometry = new THREE.BufferGeometry()
      .setAttribute('position', new THREE.Float32BufferAttribute(this.positions, 3))
      .setAttribute('color', new THREE.Float32BufferAttribute(this.colors, 3));
  }
}
