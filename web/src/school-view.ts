// The schools view (ADR 0055): homes tinted by the school areas that reach them, roads inside
// an area, and each school's site by how full it is. Display only: the sim decides every area
// and count. Also answers which school stands at a point, so a click can inspect it.
import * as THREE from 'three';
import { flat, mesh, Triangles } from './overlay-mesh.ts';
import { SCHOOL_VIEW, SERVICES } from './palette.ts';
import { SCHOOL_ROAD_STRIDE, SCHOOL_VIEW_STRIDE, SchoolArea } from './protocol.ts';

type SchoolViewMessage = { schools: Float32Array; roads: Float32Array; homes: Float32Array };

/** A school this full is shown as nearly out of seats. */
const NEARLY_FULL = 0.9;
/** Half width of the tint on a road inside a school's area. */
const ROAD_HALF = 4.5;

/** Colour for a home's `SchoolArea` bits: both kinds of school reach it, one, or none. */
export function areaColor(areas: number): number {
  const kinds = [SchoolArea.Primary, SchoolArea.High].filter((bit) => areas & bit).length;
  return kinds === 2 ? SERVICES.served : kinds === 1 ? SERVICES.short : SERVICES.without;
}

/** Colour for a school's site by seats taken: room, nearly full, or full. */
export function seatColor(seats: number, pupils: number): number {
  if (pupils >= seats) return SERVICES.without;
  return pupils >= seats * NEARLY_FULL ? SERVICES.short : SERVICES.served;
}

/** The stable building ID of the school whose square site holds `(x, y)`, or null. */
export function schoolAt(schools: Float32Array, x: number, y: number): number | null {
  for (let i = 0; i < schools.length; i += SCHOOL_VIEW_STRIDE) {
    const [cx, cy, half, fx, fy] = schools.subarray(i, i + 5);
    const [dx, dy] = [x - cx, y - cy];
    // Along the unit front and across it: the site is a square turned to face its road.
    if (Math.abs(dx * fx + dy * fy) <= half && Math.abs(dx * fy - dy * fx) <= half)
      return schools[i + 8];
  }
  return null;
}

export class SchoolView {
  readonly group = new THREE.Group();
  /** Over the zone tint, under lot lines and roads, like the services view. */
  private readonly lots = mesh(flat(0.6), 1.2, 0);
  /** Over roads and their paint, like the traffic view. */
  private readonly roads = mesh(flat(0.7), 6, 0.05);
  private schools: Float32Array = new Float32Array();

  constructor() {
    this.group.add(this.lots, this.roads);
  }

  set visible(on: boolean) {
    this.lots.visible = this.roads.visible = on;
  }

  /** The school at a ground point, as `schoolAt`. Works whether or not the view shows. */
  at(x: number, y: number): number | null {
    return schoolAt(this.schools, x, y);
  }

  /** The `schoolView` message. */
  set(view: SchoolViewMessage) {
    this.schools = view.schools;
    const lots = new Triangles();
    const { homes, schools, roads } = view;
    for (let i = 0; i < homes.length; i += 2 + 2 * homes[i + 1])
      lots.ring(homes, i + 2, homes[i + 1], new THREE.Color(areaColor(homes[i])));
    for (let i = 0; i < schools.length; i += SCHOOL_VIEW_STRIDE) {
      const [x, y, half, fx, fy, , seats, pupils] = schools.subarray(i, i + SCHOOL_VIEW_STRIDE);
      const corners = [
        [1, 1],
        [1, -1],
        [-1, -1],
        [-1, 1],
      ].flatMap(([u, v]) => [x + (u * fx - v * fy) * half, y + (u * fy + v * fx) * half]);
      lots.ring(corners, 0, 4, new THREE.Color(seatColor(seats, pupils)));
    }
    lots.upload(this.lots);
    const lines = new Triangles();
    const tint = new THREE.Color(SCHOOL_VIEW.road);
    for (let i = 0; i < roads.length; i += SCHOOL_ROAD_STRIDE) {
      const [ax, ay, bx, by] = roads.subarray(i, i + 4);
      lines.segment(ax, ay, bx, by, ROAD_HALF, tint);
    }
    lines.upload(this.roads);
  }
}
