// Where decorative scenery must give way: near roads, on zoned land and inside lots.
import { inside, type Pt } from '../building-kit/polygon.ts';

const BUCKET = 48;
/** Asphalt half-widths by `RoadClass` (street, main, alley), as drawn. */
const HALF = [4, 6, 4];
/** Roads are indexed this far past their edges, the widest margin callers ask about. */
const REACH = 12;
/** One number per bucket; distinct while bucket coordinates stay within ±2^19 (25,000 km). */
const key = (x: number, y: number) => x * 0x100000 + y;

export class Clearance {
  private readonly roads = new Map<number, number[]>();
  private segments: Float32Array = new Float32Array();
  private zones: Uint8Array | null = null;
  private layout: [number, number, number, number, number] = [0, 0, 1, 0, 0];
  private lots = new Map<number, Pt[][]>();

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
          const list = this.roads.get(key(x, y));
          if (list) list.push(i);
          else this.roads.set(key(x, y), [i]);
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
          const list = this.lots.get(key(x, y));
          if (list) list.push(ring);
          else this.lots.set(key(x, y), [ring]);
        }
    }
  }

  /** Metres from `(x, y)` to the nearest asphalt edge, exact up to `REACH`; else Infinity. */
  roadGap(x: number, y: number): number {
    const list = this.roads.get(key(Math.floor(x / BUCKET), Math.floor(y / BUCKET)));
    let best = Infinity;
    for (const i of list ?? []) best = Math.min(best, this.gap(i, x, y));
    return best;
  }

  /**
   * Whether one road comes within `margin` of both `p` and `q`, and so of every point between
   * them: the ground that close to one straight segment is convex.
   */
  holds(p: Pt, q: Pt, margin: number): boolean {
    const list = this.roads.get(key(Math.floor(p[0] / BUCKET), Math.floor(p[1] / BUCKET)));
    return !!list?.some(
      (i) => this.gap(i, p[0], p[1]) < margin && this.gap(i, q[0], q[1]) < margin,
    );
  }

  /**
   * `roadGap` for many points in the box `[x0, y0, x1, y1]`, from the roads that come within
   * `margin` of it, found once. Equal to `roadGap` wherever either is below `margin`; null
   * when no road comes that close.
   */
  gapsNear(box: number[], margin: number): ((x: number, y: number) => number) | null {
    const s = this.segments;
    const near = new Set<number>();
    for (let x = Math.floor(box[0] / BUCKET); x <= Math.floor(box[2] / BUCKET); x++)
      for (let y = Math.floor(box[1] / BUCKET); y <= Math.floor(box[3] / BUCKET); y++)
        for (const i of this.roads.get(key(x, y)) ?? []) {
          // A millimetre more than rounding could need.
          const pad = (HALF[s[i + 4]] ?? HALF[0]) + margin + 1e-3;
          if (
            Math.max(s[i], s[i + 2]) + pad >= box[0] &&
            Math.min(s[i], s[i + 2]) - pad <= box[2] &&
            Math.max(s[i + 1], s[i + 3]) + pad >= box[1] &&
            Math.min(s[i + 1], s[i + 3]) - pad <= box[3]
          )
            near.add(i);
        }
    if (near.size === 0) return null;
    const list = [...near];
    return (x, y) => {
      let best = Infinity;
      for (const i of list) best = Math.min(best, this.gap(i, x, y));
      return best;
    };
  }

  /** Metres from `(x, y)` to the asphalt of the segment at `i`. */
  private gap(i: number, x: number, y: number): number {
    const s = this.segments;
    const [ax, ay, bx, by] = [s[i], s[i + 1], s[i + 2], s[i + 3]];
    const [dx, dy] = [bx - ax, by - ay];
    const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy || 1)));
    return Math.hypot(x - ax - dx * t, y - ay - dy * t) - (HALF[s[i + 4]] ?? HALF[0]);
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
    const list = this.lots.get(key(Math.floor(x / BUCKET), Math.floor(y / BUCKET)));
    return !!list?.some((ring) => inside([x, y], ring));
  }

  /** True where wild scenery (woods, fields, meadow trees) may not stand. */
  blocked(x: number, y: number, margin: number): boolean {
    return this.roadGap(x, y) < margin || this.zoned(x, y) || this.inLot(x, y);
  }
}
