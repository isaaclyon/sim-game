// Pollution and land value over the ground (ADR 0037). The plain view shows faint haze where
// the air is dirty and stained ground where industry left its mark; the pollution views tint
// each cell by one kind, by land value, or by residents' mood (ADR 0058). Display only: the
// sim owns every number.
import * as THREE from 'three';
import { POLLUTION, POLLUTION_ALPHA } from './palette.ts';
import type { PollutionView } from './ui/views.ts';

/** The views this layer draws; the data views (ADR 0057) show the plain haze under them. */
export type PollutionLayerView = Exclude<PollutionView, 'traffic' | 'services'>;

const color = (hex: number) => new THREE.Color(hex);
const KIND = {
  air: color(POLLUTION.air),
  ground: color(POLLUTION.ground),
  noise: color(POLLUTION.noise),
};
const LAND_LOW = color(POLLUTION.landLow);
const LAND_HIGH = color(POLLUTION.landHigh);
const MOOD_LOW = color(POLLUTION.moodLow);
const MOOD_HIGH = color(POLLUTION.moodHigh);
const HAZE = color(POLLUTION.haze);
const STAIN = color(POLLUTION.stain);
/** Strongest haze and stain in the plain view, so dirty places read without an overlay. */
const HAZE_ALPHA = 0.18;
const STAIN_ALPHA = 0.45;
/** Cells this clean draw nothing in the kind views, so clean land shows the ground. */
const CLEAR = 8;

/**
 * RGBA for one cell, from its `[air, ground, noise, landValue]` bytes and its `mood` byte (see
 * `pollution` in protocol.ts).
 */
export function pollutionColor(
  view: PollutionLayerView,
  [air, ground, noise, land]: Uint8Array | readonly number[],
  mood = 0,
): [number, number, number, number] {
  const out = new THREE.Color();
  let alpha: number;
  if (view === 'happiness') {
    // Nobody lives here: show the ground.
    if (mood === 0) return [0, 0, 0, 0];
    out.copy(MOOD_LOW).lerp(MOOD_HIGH, (mood - 1) / 254);
    alpha = POLLUTION_ALPHA;
  } else if (view === 'land') {
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

const SHARED = { transparent: true, depthWrite: false, side: THREE.DoubleSide };

export class PollutionOverlay {
  readonly group = new THREE.Group();
  private mesh?: THREE.Mesh<THREE.BufferGeometry, THREE.Material>;
  /** Haze and stain are part of the scene: lit and fogged like the ground, so they darken at
   * night. The pollution views are read like a map, the same at any hour and distance. */
  private readonly lit = new THREE.MeshLambertMaterial(SHARED);
  private readonly flat = new THREE.MeshBasicMaterial({ ...SHARED, fog: false });
  private cells?: Uint8Array;
  private mood?: Uint8Array;
  private layout?: [number, number, number, number, number];
  private view: PollutionLayerView = 'plain';

  /** `cells`, `layout` and `mood` as in the `pollution` message. */
  setCells(
    cells: Uint8Array,
    layout: [number, number, number, number, number],
    mood: Uint8Array = new Uint8Array(),
  ) {
    this.cells = cells;
    this.mood = mood;
    this.layout = layout;
    this.draw();
  }

  setView(view: PollutionLayerView) {
    this.view = view;
    this.draw();
  }

  private draw() {
    const { cells, layout, mood } = this;
    if (!cells || !layout) return;
    const [x0, y0, size, cols, rows] = layout;
    const rgba = new Uint8Array(cols * rows * 4);
    for (let i = 0; i < cols * rows; i++)
      rgba.set(pollutionColor(this.view, cells.subarray(i * 4, i * 4 + 4), mood?.[i]), i * 4);
    // Smooth between cells: pollution is a gradient, unlike zoning.
    const texture = new THREE.DataTexture(rgba, cols, rows);
    texture.magFilter = THREE.LinearFilter;
    texture.minFilter = THREE.LinearFilter;
    texture.needsUpdate = true;
    if (!this.mesh) {
      this.mesh = new THREE.Mesh(new THREE.BufferGeometry(), this.lit);
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
    // The lit material needs normals; both sides draw, and each faces its viewer.
    geom.computeVertexNormals();
    this.lit.map?.dispose();
    for (const m of [this.lit, this.flat]) {
      m.map = texture;
      m.needsUpdate = true;
    }
    this.mesh.material = this.view === 'plain' ? this.lit : this.flat;
  }
}
