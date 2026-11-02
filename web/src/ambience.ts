// What the ambient sound should be at the spot the camera looks at: how loud traffic, city noise
// and birds are. Pure and read-only, from the same snapshots the renderer draws; the sim never
// hears about it. `ambient-sound.ts` turns the mix into audio.

import type { AGENT_STRIDE, AgentKind } from './protocol';

// Type-only import keeps this loadable by plain Node in tests; the types pin the values.
const STRIDE: typeof AGENT_STRIDE = 5;
const WALKER: (typeof AgentKind)['Walker'] = 1;

/** Volumes from 0 to 1 per layer. */
export interface AmbientMix {
  traffic: number;
  city: number;
  birds: number;
}

/** Where the camera listens: the sim point it looks at, and its distance from it in metres. */
export interface Listener {
  x: number;
  y: number;
  distance: number;
}

/** Buildings as `[x, y, height]` per building: footprint centre and height in metres. */
export function buildingCentres(data: Float32Array): Float32Array {
  const out: number[] = [];
  for (let i = 0; i < data.length; ) {
    // `[zone, height, roof, front, n, x0, y0, …]` per building (see protocol.ts).
    const [height, n] = [data[i + 1], data[i + 4]];
    let [x, y] = [0, 0];
    for (let k = 0; k < n; k++) {
      x += data[i + 5 + 2 * k];
      y += data[i + 6 + 2 * k];
    }
    if (n > 0) out.push(x / n, y / n, height);
    i += 5 + 2 * n;
  }
  return new Float32Array(out);
}

/** Smallest and largest hearing radius in metres around the listened-to point. */
const MIN_RADIUS = 60;
const MAX_RADIUS = 800;
/** Camera distance at which detail sounds drop to half. */
const HALF_DISTANCE = 400;
/** Weighted counts that bring a layer to about two thirds of full volume. */
const TRAFFIC_FULL = 3;
const CITY_FULL = 15;
const OPEN_FULL = 25;

const clamp01 = (v: number) => Math.min(Math.max(v, 0), 1);
/** 0 → 0, `full` → about 0.63, growing toward 1 without overshooting. */
const saturate = (sum: number, full: number) => 1 - Math.exp(-sum / full);

/** Birds sing from dawn to dusk: silent at night, fading in 05:00–06:30 and out 19:00–21:00. */
export function daylight(secondOfDay: number): number {
  const h = secondOfDay / 3600;
  return clamp01(Math.min((h - 5) / 1.5, (21 - h) / 2));
}

/**
 * The mix heard at `at`. Each car, building and walker within the hearing radius counts more
 * the closer it is; moving cars hum more than stopped ones. Birds need open, quiet land and
 * daylight. Zooming out widens the radius, so the whole city blends into a hum, and fades the
 * detail. `agents` is the agent snapshot, `buildings` comes from `buildingCentres`.
 */
export function ambientMix(
  agents: Float32Array | undefined,
  buildings: Float32Array,
  at: Listener,
  secondOfDay: number,
): AmbientMix {
  const radius = Math.min(Math.max(at.distance * 0.6, MIN_RADIUS), MAX_RADIUS);
  const near = 1 / (1 + Math.max(at.distance, 0) / HALF_DISTANCE);
  const weight = (x: number, y: number) => {
    const d = Math.hypot(x - at.x, y - at.y);
    return d < radius ? 1 - d / radius : 0;
  };
  let traffic = 0;
  let city = 0;
  if (agents) {
    for (let o = 0; o + STRIDE <= agents.length; o += STRIDE) {
      const ratio = agents[o + 3];
      if (ratio < 0) continue;
      const w = weight(agents[o], agents[o + 1]);
      if (w === 0) continue;
      if (agents[o + 4] === WALKER) city += w * 0.5;
      else traffic += w * (0.3 + 0.7 * Math.min(ratio, 1));
    }
  }
  for (let i = 0; i + 3 <= buildings.length; i += 3)
    city += weight(buildings[i], buildings[i + 1]) * Math.max(buildings[i + 2], 3) * 0.2;
  const open = Math.exp(-(traffic + city) / OPEN_FULL);
  return {
    traffic: saturate(traffic, TRAFFIC_FULL) * (0.4 + 0.6 * near),
    city: saturate(city, CITY_FULL) * (0.4 + 0.6 * near),
    birds: open * daylight(secondOfDay) * near,
  };
}
