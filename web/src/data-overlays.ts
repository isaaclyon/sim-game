// The traffic and services views (ADR 0057): roads tinted by how backed up they are, and lots
// tinted by the services their building lacks. Display only: the sim owns every number. The
// pollution views and plain haze stay in `pollution-overlay.ts`; `DataOverlays` picks between
// them by the Views list's ground overlay.
import * as THREE from 'three';
import { SERVICES, TRAFFIC } from './palette.ts';
import { PollutionOverlay } from './pollution-overlay.ts';
import { Gap, RoadClass, TRAFFIC_STRIDE } from './protocol.ts';
import type { PollutionView } from './ui/views.ts';

const FREE = new THREE.Color(TRAFFIC.free);
const SLOW = new THREE.Color(TRAFFIC.slow);
const STUCK = new THREE.Color(TRAFFIC.stuck);
/** A quarter of a lane queued already reads as slow; well over half is a jam. */
const SLOW_AT = 0.25;
const STUCK_AT = 0.6;
/** Each message moves a road's colour this far toward its new jam, so lights cycling do not
 * flash whole roads between green and red. */
const EASE = 0.5;
/** Half widths of the tint: the asphalt (see `road-geometry.ts`) plus a little. */
const HALF = { street: 4.5, main: 6.5 };

/** Colour for a road's `jam` (0 flowing to 1 stopped): green, then yellow, then red. */
export function jamColor(jam: number): THREE.Color {
  if (jam <= SLOW_AT) return FREE.clone().lerp(SLOW, jam / SLOW_AT);
  return SLOW.clone().lerp(STUCK, Math.min(1, (jam - SLOW_AT) / (STUCK_AT - SLOW_AT)));
}

/** Eases each road's shown jam toward the new `segments`; roads without cars drop out. */
export function easeJams(shown: Map<number, number>, segments: Float32Array): Map<number, number> {
  const next = new Map<number, number>();
  for (let i = 0; i < segments.length; i += TRAFFIC_STRIDE) {
    const [road, jam] = [segments[i + 5], segments[i + 6]];
    if (next.has(road)) continue;
    const was = shown.get(road);
    next.set(road, was === undefined ? jam : was + (jam - was) * EASE);
  }
  return next;
}

/** Colour for a building's `Gap` bits: every service, one missing, or more. */
export function gapColor(gaps: number): number {
  const missing = [Gap.Power, Gap.Water, Gap.Fire].filter((bit) => gaps & bit).length;
  return missing === 0 ? SERVICES.served : missing === 1 ? SERVICES.short : SERVICES.without;
}

const flat = (opacity: number) =>
  new THREE.MeshBasicMaterial({
    vertexColors: true,
    transparent: true,
    opacity,
    depthWrite: false,
    side: THREE.DoubleSide,
    fog: false,
  });

function mesh(material: THREE.Material, renderOrder: number, y: number): THREE.Mesh {
  const m = new THREE.Mesh(new THREE.BufferGeometry(), material);
  m.renderOrder = renderOrder;
  m.position.y = y;
  m.frustumCulled = false;
  m.visible = false;
  return m;
}

function upload(m: THREE.Mesh, positions: number[], colors: number[]) {
  m.geometry.dispose();
  m.geometry = new THREE.BufferGeometry()
    .setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
    .setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
}

export class DataOverlays {
  readonly group = new THREE.Group();
  private readonly pollution = new PollutionOverlay();
  /** Over roads and their paint, like the fire view's covered roads. */
  private readonly traffic = mesh(flat(0.85), 6, 0.05);
  /** Over the zone tint, under lot lines and roads, like the pollution views. */
  private readonly services = mesh(flat(0.6), 1.2, 0);
  private jams = new Map<number, number>();

  constructor() {
    this.group.add(this.pollution.group, this.traffic, this.services);
  }

  /** `cells`, `layout` and `mood` as in the `pollution` message (ADR 0037, 0058). */
  setPollution(
    cells: Uint8Array,
    layout: [number, number, number, number, number],
    mood: Uint8Array,
  ) {
    this.pollution.setCells(cells, layout, mood);
  }

  /** The traffic and services views keep the plain view's haze and stains under them. */
  setView(view: PollutionView) {
    this.pollution.setView(view === 'traffic' || view === 'services' ? 'plain' : view);
    this.traffic.visible = view === 'traffic';
    this.services.visible = view === 'services';
  }

  /** `segments` as in the `traffic` message: one quad per segment, coloured by its road. */
  setTraffic(segments: Float32Array) {
    this.jams = easeJams(this.jams, segments);
    const positions: number[] = [];
    const colors: number[] = [];
    for (let i = 0; i < segments.length; i += TRAFFIC_STRIDE) {
      const [ax, ay, bx, by, cls, road] = segments.subarray(i, i + 6);
      const length = Math.hypot(bx - ax, by - ay);
      if (length === 0) continue;
      const half = cls === RoadClass.Main ? HALF.main : HALF.street;
      // Along by `half` past each end, so bends and junctions have no gaps.
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
      const c = jamColor(this.jams.get(road) ?? 0);
      for (const k of [0, 1, 2, 0, 2, 3]) {
        positions.push(corners[k][0], 0, corners[k][1]);
        colors.push(c.r, c.g, c.b);
      }
    }
    upload(this.traffic, positions, colors);
  }

  /** `lots` as in the `services` message: each building's lot, filled by `gapColor`. */
  setServices(lots: Float32Array) {
    const positions: number[] = [];
    const colors: number[] = [];
    for (let i = 0; i < lots.length; ) {
      const c = new THREE.Color(gapColor(lots[i]));
      const n = lots[i + 1];
      const ring: THREE.Vector2[] = [];
      for (let k = 0; k < n; k++)
        ring.push(new THREE.Vector2(lots[i + 2 + 2 * k], lots[i + 3 + 2 * k]));
      i += 2 + 2 * n;
      // triangulateShape may reverse the ring; its indices point into the array as left.
      for (const tri of THREE.ShapeUtils.triangulateShape(ring, []))
        for (const k of tri) {
          positions.push(ring[k].x, 0, ring[k].y);
          colors.push(c.r, c.g, c.b);
        }
    }
    upload(this.services, positions, colors);
  }
}
