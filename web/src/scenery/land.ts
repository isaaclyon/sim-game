// Decorative land around and under the city: ground patches, a river, a lake, woods and
// fields. Placed from the map extent alone; it never affects the sim (issue #27). Water and
// trees will become sim-owned later, when they start to block roads.
import type { Pt } from '../building-kit/polygon.ts';
import { fbm, rng } from './noise.ts';

export type Patch = 'grass' | 'grassLight' | 'grassDark' | 'meadow' | 'dirt' | 'woodsFloor';
export type Crop = 'fieldWheat' | 'fieldCrop' | 'fieldPlough' | 'meadow';

export interface Field {
  /** Corners in order; strips run parallel to the first edge. */
  corners: [Pt, Pt, Pt, Pt];
  crop: Crop;
  /** Points along the far edge where a hedgerow grows, or empty. */
  hedge: Pt[];
}

export const RIVER_HALF = 19;

export class Land {
  readonly extent: [number, number, number, number];
  readonly fields: Field[];
  private readonly lakeCentre: Pt;
  private readonly lakeRadius: number;

  /** `extent` is `[x0, y0, x1, y1]` in sim metres, including any margin around the map. */
  constructor(extent: [number, number, number, number]) {
    this.extent = extent;
    const [x0, y0, x1, y1] = extent;
    this.lakeCentre = [x1 - (x1 - x0) * 0.14, y0 + (y1 - y0) * 0.16];
    this.lakeRadius = Math.max(140, Math.min(x1 - x0, y1 - y0) * 0.03);
    this.fields = this.placeFields();
  }

  /** The river's centre line: a meander running across the map's west side. */
  riverX(y: number): number {
    const [x0, , x1] = this.extent;
    return x0 + (x1 - x0) * 0.2 + 70 * Math.sin(y / 260) + 30 * Math.sin(y / 90 + 1);
  }

  lakeR(angle: number): number {
    const r = this.lakeRadius;
    return r + r * 0.14 * Math.sin(angle * 3 + 1) + r * 0.13 * Math.sin(angle * 5 + 2);
  }

  /** The lake's shore pushed out by `d` metres, as a closed ring. */
  lakeRing(d = 0, sides = 64): Pt[] {
    return Array.from({ length: sides }, (_, k) => {
      const a = (k / sides) * Math.PI * 2;
      const r = this.lakeR(a) + d;
      return [this.lakeCentre[0] + Math.cos(a) * r, this.lakeCentre[1] + Math.sin(a) * r];
    });
  }

  /** Metres from the nearest shore; negative in the water. */
  waterDistance(x: number, y: number): number {
    const river = Math.abs(x - this.riverX(y)) - RIVER_HALF;
    const [dx, dy] = [x - this.lakeCentre[0], y - this.lakeCentre[1]];
    const lake = Math.hypot(dx, dy) - this.lakeR(Math.atan2(dy, dx));
    return Math.min(river, lake);
  }

  woods(x: number, y: number): boolean {
    return this.waterDistance(x, y) > 14 && fbm(x / 330 + 11, y / 330 + 4, 9) > 0.68;
  }

  patch(x: number, y: number): Patch {
    if (this.woods(x, y)) return 'woodsFloor';
    if (fbm(x / 140, y / 140, 5) > 0.79) return 'dirt';
    const v = fbm(x / 380 + 3, y / 380 - 2, 3);
    return v < 0.36 ? 'grassDark' : v < 0.56 ? 'grass' : v < 0.68 ? 'grassLight' : 'meadow';
  }

  /** Rectangular fields in clusters where a farmland noise field is high, off water and woods. */
  private placeFields(): Field[] {
    const [x0, y0, x1, y1] = this.extent;
    const r = rng(77);
    const out: Field[] = [];
    const step = 150;
    for (let y = y0 + step / 2; y < y1 - step; y += step)
      for (let x = x0 + step / 2; x < x1 - step; x += step) {
        if (fbm(x / 900 + 21, y / 900 - 13, 12) < 0.55 || r() < 0.2) continue;
        const [w, d] = [80 + r() * 60, 80 + r() * 60];
        const corners: [Pt, Pt, Pt, Pt] = [
          [x, y],
          [x + w, y],
          [x + w, y + d],
          [x, y + d],
        ];
        const clear = [...corners, [x + w / 2, y + d / 2] as Pt].every(
          ([px, py]) => this.waterDistance(px, py) > 30 && !this.woods(px, py),
        );
        if (!clear) continue;
        const t = r();
        const crop: Crop =
          t < 0.35 ? 'fieldWheat' : t < 0.65 ? 'fieldCrop' : t < 0.85 ? 'fieldPlough' : 'meadow';
        const hedge: Pt[] = [];
        if (r() < 0.5)
          for (let a = 4; a < w; a += 9) if (r() < 0.75) hedge.push([x + a, y + d + 4]);
        out.push({ corners, crop, hedge });
      }
    return out;
  }
}
