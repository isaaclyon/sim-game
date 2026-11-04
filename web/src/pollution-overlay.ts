// Pollution and land value over the ground (ADR 0036). The plain view shows faint haze where
// the air is dirty and stained ground where industry left its mark; the pollution views tint
// each cell by one kind, or by land value. Display only: the sim owns every number.
import * as THREE from 'three';
import { POLLUTION, POLLUTION_ALPHA } from './palette.ts';
import type { PollutionView } from './view-buttons.ts';

const color = (hex: number) => new THREE.Color(hex);
const KIND = {
  air: color(POLLUTION.air),
  ground: color(POLLUTION.ground),
  noise: color(POLLUTION.noise),
};
const LAND_LOW = color(POLLUTION.landLow);
const LAND_HIGH = color(POLLUTION.landHigh);
const HAZE = color(POLLUTION.haze);
const STAIN = color(POLLUTION.stain);
/** Strongest haze and stain in the plain view, so dirty places read without an overlay. */
const HAZE_ALPHA = 0.18;
const STAIN_ALPHA = 0.45;
/** Cells this clean draw nothing in the kind views, so clean land shows the ground. */
const CLEAR = 8;

/**
 * RGBA for one cell, from its `[air, ground, noise, landValue]` bytes (see `pollution` in
 * protocol.ts).
 */
export function pollutionColor(
  view: PollutionView,
  [air, ground, noise, land]: Uint8Array | readonly number[],
): [number, number, number, number] {
  const out = new THREE.Color();
  let alpha: number;
  if (view === 'land') {
    out.copy(LAND_LOW).lerp(LAND_HIGH, land / 255);
    alpha = POLLUTION_ALPHA;
  } else if (view === 'plain') {
    // Stain over haze: old factory ground should read even under clean air.
    const [h, s] = [(air / 255) * HAZE_ALPHA, (ground / 255) * STAIN_ALPHA];
    alpha = h + s - h * s;
    if (alpha > 0) out.copy(HAZE).lerp(STAIN, s / alpha);
  } else {
    const level = { air, ground, noise }[view];
    if (level < CLEAR) return [0, 0, 0, 0];
    out.copy(KIND[view]);
    // A floor so faint pollution still reads against bright ground.
    alpha = (0.3 + 0.7 * Math.sqrt(level / 255)) * POLLUTION_ALPHA;
  }
  return [out.r * 255, out.g * 255, out.b * 255, alpha * 255];
}

export class PollutionOverlay {
  readonly group = new THREE.Group();
  private mesh?: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  private cells?: Uint8Array;
  private layout?: [number, number, number, number, number];
  private view: PollutionView = 'plain';

  /** `cells` and `layout` as in the `pollution` message. */
  setCells(cells: Uint8Array, layout: [number, number, number, number, number]) {
    this.cells = cells;
    this.layout = layout;
    this.draw();
  }

  setView(view: PollutionView) {
    this.view = view;
    this.draw();
  }

  private draw() {
    const { cells, layout } = this;
    if (!cells || !layout) return;
    const [x0, y0, size, cols, rows] = layout;
    const rgba = new Uint8Array(cols * rows * 4);
    for (let i = 0; i < cols * rows; i++)
      rgba.set(pollutionColor(this.view, cells.subarray(i * 4, i * 4 + 4)), i * 4);
    // Smooth between cells: pollution is a gradient, unlike zoning.
    const texture = new THREE.DataTexture(rgba, cols, rows);
    texture.magFilter = THREE.LinearFilter;
    texture.minFilter = THREE.LinearFilter;
    texture.needsUpdate = true;
    if (!this.mesh) {
      // No fog: an overlay is read like a map and should not fade with distance.
      const material = new THREE.MeshBasicMaterial({
        fog: false,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
      });
      this.mesh = new THREE.Mesh(new THREE.BufferGeometry(), material);
      // Over the zone tint, under lot lines and roads.
      this.mesh.renderOrder = 1.2;
      this.mesh.frustumCulled = false;
      this.group.add(this.mesh);
    }
    const [x1, y1] = [x0 + cols * size, y0 + rows * size];
    const geom = this.mesh.geometry;
    geom.setAttribute(
      'position',
      new THREE.Float32BufferAttribute([x0, 0, y0, x1, 0, y0, x1, 0, y1, x0, 0, y1], 3),
    );
    geom.setAttribute('uv', new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2));
    geom.setIndex([0, 1, 2, 0, 2, 3]);
    this.mesh.material.map?.dispose();
    this.mesh.material.map = texture;
    this.mesh.material.needsUpdate = true;
  }
}
