// Where decorative scenery must give way: near roads, on zoned land and inside lots.
import { inside, type Pt } from '../building-kit/polygon.ts';

const BUCKET = 48;
/** Asphalt half-widths by `RoadClass` (street, main, alley), as drawn. */
const HALF = [4, 6, 4];
/** Roads are indexed this far past their edges, the widest margin callers ask about. */
const REACH = 12;

export class Clearance {
  private readonly roads = new Map<string, number[]>();
  private segments: Float32Array = new Float32Array();
  private zones: Uint8Array | null = null;
  private layout: [number, number, number, number, number] = [0, 0, 1, 0, 0];
  private lots = new Map<string, Pt[][]>();

  /** `[ax, ay, bx, by, class]` per rendered road segment. */
  setRoads(segments: Float32Array) {
    this.segments = segments;
    this.roads.clear();
    for (let i = 0; i < segments.length; i += 5) {
      const [ax, ay, bx, by] = segments.subarray(i, i + 4);
      const pad = (HALF[segments[i + 4]] ?? HALF[0]) + REACH;
      const [x0, x1] = [Math.min(ax, bx) - pad, Math.max(ax, bx) + pad];
      const [y0, y1] = [Math.min(ay, by) - pad, Math.max(ay, by) + pad];
      for (let x = Math.floor(x0 / BUCKET); x <= Math.floor(x1 / BUCKET); x++)
        for (let y = Math.floor(y0 / BUCKET); y <= Math.floor(y1 / BUCKET); y++) {
          const key = `${x},${y}`;
          const list = this.roads.get(key);
          if (list) list.push(i);
          else this.roads.set(key, [i]);
        }
    }
  }

  setZones(cells: Uint8Array, layout: [number, number, number, number, number]) {
    this.zones = cells;
    this.layout = layout;
  }

  /** `[n, x0, y0, …]` per lot. */
  setLots(rings: Float32Array) {
    this.lots.clear();
    for (let i = 0; i < rings.length; ) {
      const n = rings[i];
      const ring: Pt[] = [];
      for (let k = 0; k < n; k++) ring.push([rings[i + 1 + 2 * k], rings[i + 2 + 2 * k]]);
      i += 1 + 2 * n;
      const xs = ring.map((p) => Math.floor(p[0] / BUCKET));
      const ys = ring.map((p) => Math.floor(p[1] / BUCKET));
      for (let x = Math.min(...xs); x <= Math.max(...xs); x++)
        for (let y = Math.min(...ys); y <= Math.max(...ys); y++) {
          const key = `${x},${y}`;
          const list = this.lots.get(key);
          if (list) list.push(ring);
          else this.lots.set(key, [ring]);
        }
    }
  }

  /** Metres from `(x, y)` to the nearest asphalt edge, exact up to `REACH`; else Infinity. */
  roadGap(x: number, y: number): number {
    const list = this.roads.get(`${Math.floor(x / BUCKET)},${Math.floor(y / BUCKET)}`);
    let best = Infinity;
    for (const i of list ?? []) {
      const s = this.segments;
      const [ax, ay, bx, by] = [s[i], s[i + 1], s[i + 2], s[i + 3]];
      const [dx, dy] = [bx - ax, by - ay];
      const t = Math.max(
        0,
        Math.min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy || 1)),
      );
      const d = Math.hypot(x - ax - dx * t, y - ay - dy * t) - (HALF[s[i + 4]] ?? HALF[0]);
      best = Math.min(best, d);
    }
    return best;
  }

  private zoneAt(x: number, y: number): number {
    const [x0, y0, size, cols, rows] = this.layout;
    const [c, r] = [Math.floor((x - x0) / size), Math.floor((y - y0) / size)];
    if (!this.zones || c < 0 || r < 0 || c >= cols || r >= rows) return 0;
    return this.zones[r * cols + c];
  }

  zoned(x: number, y: number): boolean {
    return this.zoneAt(x, y) !== 0;
  }

  /** Zone values 1–4: residential or commercial, low or high density. */
  residentialOrCommercial(x: number, y: number): boolean {
    const zone = this.zoneAt(x, y);
    return zone >= 1 && zone <= 4;
  }

  inLot(x: number, y: number): boolean {
    const list = this.lots.get(`${Math.floor(x / BUCKET)},${Math.floor(y / BUCKET)}`);
    return !!list?.some((ring) => inside([x, y], ring));
  }

  /** True where wild scenery (woods, fields, meadow trees) may not stand. */
  blocked(x: number, y: number, margin: number): boolean {
    return this.roadGap(x, y) < margin || this.zoned(x, y) || this.inLot(x, y);
  }
}
