// The traffic and services views (ADR 0057): roads tinted by how backed up they are, and lots
// tinted by the services their building lacks; the schools view lives in `school-view.ts`. Display only: the sim owns every number. The
// pollution views and plain haze stay in `pollution-overlay.ts`; `DataOverlays` picks between
// them by the Views list's ground overlay.
import * as THREE from 'three';
import { flat, mesh, Triangles } from './overlay-mesh.ts';
import { SERVICES, TRAFFIC } from './palette.ts';
import { PollutionOverlay } from './pollution-overlay.ts';
import { Gap, RoadClass, TRAFFIC_STRIDE } from './protocol.ts';
import { SchoolView } from './school-view.ts';
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

export class DataOverlays {
  readonly group = new THREE.Group();
  private readonly pollution = new PollutionOverlay();
  /** Over roads and their paint, like the fire view's covered roads. */
  private readonly traffic = mesh(flat(0.85), 6, 0.05);
  /** Over the zone tint, under lot lines and roads, like the pollution views. */
  private readonly services = mesh(flat(0.6), 1.2, 0);
  /** The schools view (ADR 0055); it also knows which school stands where. */
  readonly schools = new SchoolView();
  private jams = new Map<number, number>();

  constructor() {
    this.group.add(this.pollution.group, this.traffic, this.services, this.schools.group);
  }

  /** `cells`, `layout` and `mood` as in the `pollution` message (ADR 0037, 0058). */
  setPollution(
    cells: Uint8Array,
    layout: [number, number, number, number, number],
    mood: Uint8Array,
  ) {
    this.pollution.setCells(cells, layout, mood);
  }

  /** The traffic, services and schools views keep the plain view's haze and stains under
   * them. */
  setView(view: PollutionView) {
    const data = view === 'traffic' || view === 'services' || view === 'schools';
    this.pollution.setView(data ? 'plain' : view);
    this.schools.visible = view === 'schools';
    this.traffic.visible = view === 'traffic';
    this.services.visible = view === 'services';
  }

  /** `segments` as in the `traffic` message: one quad per segment, coloured by its road. */
  setTraffic(segments: Float32Array) {
    this.jams = easeJams(this.jams, segments);
    const quads = new Triangles();
    for (let i = 0; i < segments.length; i += TRAFFIC_STRIDE) {
      const [ax, ay, bx, by, cls, road] = segments.subarray(i, i + 6);
      const half = cls === RoadClass.Main ? HALF.main : HALF.street;
      quads.segment(ax, ay, bx, by, half, jamColor(this.jams.get(road) ?? 0));
    }
    quads.upload(this.traffic);
  }

  /** `lots` as in the `services` message: each building's lot, filled by `gapColor`. */
  setServices(lots: Float32Array) {
    const fills = new Triangles();
    for (let i = 0; i < lots.length; i += 2 + 2 * lots[i + 1])
      fills.ring(lots, i + 2, lots[i + 1], new THREE.Color(gapColor(lots[i])));
    fills.upload(this.services);
  }
}
