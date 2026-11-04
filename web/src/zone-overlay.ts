// The zone grid as a translucent tint over the ground, and lot edges over it. Both draw in
// the transparent pass after every opaque mesh, then roads cover them. Freshly painted cells
// flash a lighter shade of their new zone (white when unzoned), then fade to the plain tint.
import * as THREE from 'three';
import { ZONE_OVERLAY, ZONE_OVERLAY_ALPHA } from './palette.ts';

const ZONE_COLORS = ZONE_OVERLAY.map((c) => (c === null ? null : new THREE.Color(c)));
const LOT_COLOR = 0x5d6656;
const WHITE = new THREE.Color(0xffffff);
/** How long painted cells glow, in seconds, and how strongly at first. */
export const FLASH_SECONDS = 0.6;
const FLASH_PEAK = 0.75;

export class ZoneOverlay {
  readonly group = new THREE.Group();
  private zones?: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  private lots?: THREE.LineSegments;
  private flash?: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  private flashStart = 0;
  private cells?: Uint8Array;

  constructor() {
    this.group.visible = false;
  }

  /**
   * `cells` is one `Zone` byte per cell, row-major; `layout` is
   * `[originX, originY, cellSize, cols, rows]` (see `zones` in protocol.ts). With `now`
   * (seconds), cells that changed since the last grid flash from that time.
   */
  setZones(
    cells: Uint8Array,
    layout: [number, number, number, number, number],
    now: number | null = null,
  ) {
    const [x0, y0, size, cols, rows] = layout;
    const rgba = new Uint8Array(cols * rows * 4);
    const alpha = ZONE_OVERLAY_ALPHA * 255;
    for (let i = 0; i < cells.length; i++) {
      const c = ZONE_COLORS[cells[i]];
      if (!c) continue;
      rgba.set([c.r * 255, c.g * 255, c.b * 255, alpha], i * 4);
    }
    // Row 0 of the texture is the grid's first row, at v = 0, which the geometry puts at y0.
    const texture = new THREE.DataTexture(rgba, cols, rows);
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.needsUpdate = true;
    if (!this.zones) {
      const [x1, y1] = [x0 + cols * size, y0 + rows * size];
      const geom = new THREE.BufferGeometry();
      geom.setAttribute(
        'position',
        new THREE.Float32BufferAttribute([x0, 0, y0, x1, 0, y0, x1, 0, y1, x0, 0, y1], 3),
      );
      geom.setAttribute('uv', new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2));
      geom.setIndex([0, 1, 2, 0, 2, 3]);
      // Unzoned texels are discarded; zoned ones blend over the ground.
      const material = new THREE.MeshBasicMaterial({
        alphaTest: 0.05,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
      });
      this.zones = new THREE.Mesh(geom, material);
      this.zones.renderOrder = 1;
      this.zones.frustumCulled = false;
      this.group.add(this.zones);
    }
    this.zones.material.map?.dispose();
    this.zones.material.map = texture;
    this.zones.material.needsUpdate = true;
    const before = this.cells;
    this.cells = cells.slice();
    if (now === null || before?.length !== cells.length) return;
    const glow = new Uint8Array(cols * rows * 4);
    const c = new THREE.Color();
    let changed = false;
    for (let i = 0; i < cells.length; i++) {
      if (cells[i] === before[i]) continue;
      changed = true;
      c.copy(ZONE_COLORS[cells[i]] ?? WHITE).lerp(WHITE, 0.5);
      glow.set([c.r * 255, c.g * 255, c.b * 255, 255], i * 4);
    }
    if (!changed) return;
    const flashTexture = new THREE.DataTexture(glow, cols, rows);
    flashTexture.magFilter = THREE.NearestFilter;
    flashTexture.minFilter = THREE.NearestFilter;
    flashTexture.needsUpdate = true;
    if (!this.flash) {
      const material = new THREE.MeshBasicMaterial({
        alphaTest: 0.02,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
      });
      this.flash = new THREE.Mesh(this.zones.geometry, material);
      this.flash.renderOrder = 1.5;
      this.flash.frustumCulled = false;
      this.group.add(this.flash);
    }
    this.flash.material.map?.dispose();
    this.flash.material.map = flashTexture;
    this.flash.material.opacity = FLASH_PEAK;
    this.flash.material.needsUpdate = true;
    this.flash.visible = true;
    this.flashStart = now;
  }

  /** Fades the paint flash at `now` (seconds); true while it still shows. */
  animate(now: number): boolean {
    if (!this.flash?.visible) return false;
    const p = (now - this.flashStart) / FLASH_SECONDS;
    this.flash.visible = p < 1;
    this.flash.material.opacity = FLASH_PEAK * (1 - Math.min(Math.max(p, 0), 1)) ** 2;
    return this.flash.visible;
  }

  /** `[n, x0, y0, …]` per lot (see `lots` in protocol.ts). */
  setLots(rings: Float32Array) {
    const pos: number[] = [];
    for (let i = 0; i < rings.length; ) {
      const n = rings[i];
      const at = (k: number) => i + 1 + 2 * (k % n);
      for (let k = 0; k < n; k++) {
        const [a, b] = [at(k), at(k + 1)];
        pos.push(rings[a], 0.1, rings[a + 1], rings[b], 0.1, rings[b + 1]);
      }
      i += 1 + 2 * n;
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(pos), 3));
    if (!this.lots) {
      const material = new THREE.LineBasicMaterial({
        color: LOT_COLOR,
        transparent: true,
        opacity: 0.35,
        depthWrite: false,
      });
      this.lots = new THREE.LineSegments(geom, material);
      this.lots.renderOrder = 2;
      this.lots.frustumCulled = false;
      this.group.add(this.lots);
    } else {
      this.lots.geometry.dispose();
      this.lots.geometry = geom;
    }
  }
}
