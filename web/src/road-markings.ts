// Lane paint by road class, display only. Every sim road is two-way with one lane each side
// (lane centres at ±2 m), so the paint says which class a road is:
// - main road: double solid yellow centre line, white edge lines where the shoulder starts;
// - street: one dashed yellow centre line;
// - alley: no paint.
// Paint stops short of junctions where three or more roads meet, so crossings read as open.
import * as THREE from 'three';
import { MARKINGS } from './palette.ts';

const LINE = 0.15;
const EDGE_LINE = 0.2;
const DOUBLE_GAP = 0.2;
/** Lanes end 4 m out; the edge line sits just past them, inside main roads' shoulders. */
const EDGE = 4.1;
const DASH = 3;
const DASH_PERIOD = 9;
/** Matches the junction pads in road-geometry.ts: the widest road's half-width, circumscribed. */
const MAIN_HALF = 6;
const STREET_HALF = 4;
const PAD_SIDES = 12;
// `RoadClass` values, inlined so this module loads in Node tests without a bundler.
const STREET = 0;
const MAIN = 1;

const CENTRE = new THREE.Color(MARKINGS.centre);
const EDGE_PAINT = new THREE.Color(MARKINGS.edge);

/** Paint quads from `[ax, ay, bx, by, class]` per segment, with vertex colours. */
export function buildRoadMarkings(segments: Float32Array): THREE.BufferGeometry {
  const positions: number[] = [];
  const rgb: number[] = [];

  // Junction radius per shared endpoint where three or more segments meet.
  const ends = new Map<string, { count: number; half: number }>();
  for (let i = 0; i < segments.length; i += 5) {
    const half = segments[i + 4] === MAIN ? MAIN_HALF : STREET_HALF;
    for (const k of [0, 2]) {
      const key = `${segments[i + k]},${segments[i + k + 1]}`;
      const e = ends.get(key) ?? { count: 0, half: 0 };
      e.count++;
      e.half = Math.max(e.half, half);
      ends.set(key, e);
    }
  }
  const trim = (x: number, z: number) => {
    const e = ends.get(`${x},${z}`);
    return e && e.count >= 3 ? e.half / Math.cos(Math.PI / PAD_SIDES) : 0;
  };

  let chain = 0;
  let last: number[] = [];
  for (let i = 0; i < segments.length; i += 5) {
    const [ax, az, bx, bz, cls] = segments.subarray(i, i + 5);
    const length = Math.hypot(bx - ax, bz - az);
    // Curves arrive as consecutive samples; carry the dash rhythm across them.
    const continues = last[0] === ax && last[1] === az && last[2] === cls;
    const start = continues ? chain : 0;
    chain = start + length;
    last = [bx, bz, cls];
    if (length === 0 || (cls !== MAIN && cls !== STREET)) continue;
    const ux = (bx - ax) / length;
    const uz = (bz - az) / length;
    const t0 = trim(ax, az);
    const t1 = length - trim(bx, bz);
    if (t1 <= t0) continue;

    /** A strip from `s0` to `s1` metres along the segment, centred `offset` metres to its left. */
    const strip = (s0: number, s1: number, offset: number, width: number, color: THREE.Color) => {
      const [lo, hi] = [offset - width / 2, offset + width / 2];
      const at = (s: number, o: number) => [ax + ux * s - uz * o, 0, az + uz * s + ux * o];
      // Counterclockwise from above, like the road ribbons: front faces point up.
      positions.push(...at(s0, hi), ...at(s1, hi), ...at(s0, lo));
      positions.push(...at(s0, lo), ...at(s1, hi), ...at(s1, lo));
      for (let v = 0; v < 6; v++) rgb.push(color.r, color.g, color.b);
    };

    if (cls === MAIN) {
      for (const o of [-DOUBLE_GAP, DOUBLE_GAP]) strip(t0, t1, o, LINE, CENTRE);
      for (const o of [-EDGE, EDGE]) strip(t0, t1, o, EDGE_LINE, EDGE_PAINT);
      continue;
    }
    for (let k = Math.floor((start + t0) / DASH_PERIOD); k * DASH_PERIOD < start + t1; k++) {
      const s0 = Math.max(k * DASH_PERIOD - start, t0);
      const s1 = Math.min(k * DASH_PERIOD + DASH - start, t1);
      if (s1 > s0) strip(s0, s1, 0, LINE, CENTRE);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('color', new THREE.Float32BufferAttribute(rgb, 3));
  return geometry;
}
