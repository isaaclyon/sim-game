// What a tool would do, drawn over the city: a road-wide translucent ribbon along its centre
// line, snap markers, and the zone brush outline. The sim decides every shape; this module only
// draws its answers, and lets snap markers glide between answers instead of jumping.
import * as THREE from 'three';
import type { Spot } from './power-tools.ts';
import { buildRoadGeometry } from './road-geometry.ts';

export const PREVIEW_COLOR = new THREE.Color(0x4a8bd0);
export const PREVIEW_REFUSED_COLOR = new THREE.Color(0xe5484d);
/** Snap markers, indexed by join: new end (not drawn), existing node, split road. */
const JOIN_COLORS = [null, new THREE.Color(0xffffff), new THREE.Color(0xf5c542)];
/** The hover marker before a road has a start: new ends get one too, so the start is visible. */
const JOIN_MARKER = [PREVIEW_COLOR, JOIN_COLORS[1], JOIN_COLORS[2]];
const MARKER_PX = 10;
/** Bright versions for the drag outline, which must stand out. */
const ZONE_OUTLINE = [0xe5484d, 0x7bd88f, 0x2e9e4f, 0x6fb3ff, 0x2f6fd0, 0xf2c94c, 0xd9822b];
/** Markers close this share of the remaining distance in 1/`GLIDE_RATE` seconds (about 63 %). */
const GLIDE_RATE = 30;
const GUIDE_COLOR = 0xc9d4df;
/** Angle marks (ADR 0038): an arc this far from the junction, or a square this wide at 90°. */
const ARC_M = 14;
const SQUARE_M = 9;
const ARC_STEPS = 24;
/** Within this sine of square, a mark shows the right-angle square instead of an arc. */
const RIGHT_SIN = 0.0087;
const MARK_COLOR = new THREE.Color(0xc9d4df);
const RIGHT_COLOR = new THREE.Color(0x7bd88f);
const MARK_OPACITY = 0.3;
const LINE_Y = 0.5;
const RIBBON_Y = 0.3;
const RIBBON_OPACITY = 0.35;

/** A ground position: x and z in world space, sim x and y. */
export type Ground = { x: number; z: number };

export class ToolPreview {
  readonly group = new THREE.Group();
  readonly line: THREE.Line;
  readonly ribbon: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  readonly markers: THREE.Points;
  readonly zoneDrag: THREE.LineLoop;
  /** Dashed guide lines a drawn point snapped to, from each line's origin to the point. */
  readonly guides: THREE.LineSegments<THREE.BufferGeometry, THREE.LineDashedMaterial>;
  /** Angles where the road joins roads: translucent wedges or squares, and their outlines. */
  readonly angleFill: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  readonly angleEdge: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  /** Where the sim's latest answer puts each marker; drawn positions glide toward it. */
  private markerTarget = new Float32Array();

  constructor() {
    this.line = new THREE.Line(
      new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: PREVIEW_COLOR }),
    );
    this.line.visible = false;
    this.ribbon = new THREE.Mesh(
      new THREE.BufferGeometry(),
      new THREE.MeshBasicMaterial({
        color: PREVIEW_COLOR,
        transparent: true,
        opacity: RIBBON_OPACITY,
        // Overlapping pads and ribbons sit at one height; Less rejects the second layer, so
        // joints are not darker. Buildings still hide what is behind them.
        depthFunc: THREE.LessDepth,
      }),
    );
    this.ribbon.position.y = RIBBON_Y;
    this.ribbon.renderOrder = 6;
    this.ribbon.visible = false;
    this.markers = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({ size: MARKER_PX, sizeAttenuation: false, vertexColors: true }),
    );
    this.zoneDrag = new THREE.LineLoop(new THREE.BufferGeometry(), new THREE.LineBasicMaterial());
    this.zoneDrag.visible = false;
    this.guides = new THREE.LineSegments(
      new THREE.BufferGeometry(),
      new THREE.LineDashedMaterial({ color: GUIDE_COLOR, dashSize: 4, gapSize: 3 }),
    );
    this.guides.visible = false;
    this.angleFill = new THREE.Mesh(
      new THREE.BufferGeometry(),
      new THREE.MeshBasicMaterial({
        vertexColors: true,
        transparent: true,
        opacity: MARK_OPACITY,
        side: THREE.DoubleSide,
        depthWrite: false,
      }),
    );
    this.angleFill.renderOrder = 7;
    this.angleEdge = new THREE.LineSegments(
      new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ vertexColors: true }),
    );
    const parts = [this.line, this.ribbon, this.markers, this.zoneDrag, this.guides];
    for (const o of [...parts, this.angleFill, this.angleEdge]) {
      o.frustumCulled = false;
      this.group.add(o);
    }
  }

  /** See `Renderer.setPreview`. `roadClass` sets the ribbon width. */
  setRoad(stops: Float32Array | null, a: Ground | null, b: Ground | null, roadClass = 0) {
    const line: number[] = [];
    const markers: number[] = [];
    const colors: number[] = [];
    let color = PREVIEW_COLOR;
    if (stops && stops.length > 0) {
      for (let i = 0; i < stops.length; i += 3) {
        line.push(stops[i], stops[i + 1]);
        const c = stops.length === 3 ? JOIN_MARKER[stops[2]] : JOIN_COLORS[stops[i + 2]];
        if (!c) continue;
        markers.push(stops[i], 1, stops[i + 1]);
        colors.push(c.r, c.g, c.b);
      }
    } else if (stops && a && b) {
      color = PREVIEW_REFUSED_COLOR;
      line.push(a.x, a.z, b.x, b.z);
    }
    // A road ends at a join; a curve's control guide ends at -1 and is not a road.
    const road = !!stops && (stops.length === 0 || stops[stops.length - 1] >= 0);
    this.setPath(line, color, road ? roadClass : null);
    this.setMarkers(markers, colors);
  }

  /** `[x0, y0, x1, y1]` per guide line; null or empty hides them. */
  setGuides(lines: Float32Array | null) {
    const points: THREE.Vector3[] = [];
    for (let i = 0; lines && i + 3 < lines.length; i += 4)
      points.push(
        new THREE.Vector3(lines[i], LINE_Y, lines[i + 1]),
        new THREE.Vector3(lines[i + 2], LINE_Y, lines[i + 3]),
      );
    this.guides.geometry.dispose();
    this.guides.geometry = new THREE.BufferGeometry().setFromPoints(points);
    this.guides.computeLineDistances();
    this.guides.visible = points.length > 0;
  }

  /**
   * `[x, y, rx, ry, dx, dy]` per angle mark: the junction, then unit directions along the road
   * measured from and along the new road. Square marks get a right-angle square, others an
   * arc. Null or empty hides them.
   */
  setAngles(marks: Float32Array | null) {
    const fill: number[] = [];
    const edge: number[] = [];
    const fillColor: number[] = [];
    const edgeColor: number[] = [];
    const v = (x: number, y: number) => [x, LINE_Y, y];
    for (let i = 0; marks && i + 5 < marks.length; i += 6) {
      const [x, y, rx, ry, dx, dy] = marks.subarray(i, i + 6);
      const right = Math.abs(rx * dx + ry * dy) < RIGHT_SIN;
      const c = right ? RIGHT_COLOR : MARK_COLOR;
      // The outline: points from the road measured from round to the new road.
      const rim: number[][] = [];
      if (right) {
        const s = SQUARE_M;
        rim.push(v(x + rx * s, y + ry * s), v(x + (rx + dx) * s, y + (ry + dy) * s));
        rim.push(v(x + dx * s, y + dy * s));
      } else {
        const from = Math.atan2(ry, rx);
        const sweep = Math.atan2(rx * dy - ry * dx, rx * dx + ry * dy);
        for (let k = 0; k <= ARC_STEPS; k++) {
          const t = from + (sweep * k) / ARC_STEPS;
          rim.push(v(x + Math.cos(t) * ARC_M, y + Math.sin(t) * ARC_M));
        }
      }
      const centre = v(x, y);
      for (let k = 1; k < rim.length; k++) {
        fill.push(...centre, ...rim[k - 1], ...rim[k]);
        edge.push(...rim[k - 1], ...rim[k]);
        for (let j = 0; j < 3; j++) fillColor.push(c.r, c.g, c.b);
        for (let j = 0; j < 2; j++) edgeColor.push(c.r, c.g, c.b);
      }
    }
    const geometry = (xyz: number[], rgb: number[]) => {
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.Float32BufferAttribute(xyz, 3));
      g.setAttribute('color', new THREE.Float32BufferAttribute(rgb, 3));
      return g;
    };
    this.angleFill.geometry.dispose();
    this.angleFill.geometry = geometry(fill, fillColor);
    this.angleEdge.geometry.dispose();
    this.angleEdge.geometry = geometry(edge, edgeColor);
    this.angleFill.visible = this.angleEdge.visible = edge.length > 0;
  }

  /** What the bulldozer would remove, `[x, y]` per sample (a road's centreline, or a plant's or
   * building's closed outline), in the refusal red. */
  setBulldoze(segment: Float32Array) {
    this.setMarkers([], []);
    this.setPath([...segment], PREVIEW_REFUSED_COLOR, 0);
  }

  /** The road a click would change to `roadClass`, `[x, y]` per centreline sample, at that
   * class's width: blue when the city can pay, red when not (ADR 0050). */
  setUpgrade(segment: Float32Array, ok: boolean, roadClass: number) {
    this.setMarkers([], []);
    this.setPath([...segment], ok ? PREVIEW_COLOR : PREVIEW_REFUSED_COLOR, roadClass);
  }

  /** A plant's footprint, closed `[x, y]`, blue where it fits and red where it does not. */
  setPlant(outline: Float32Array, ok: boolean) {
    this.setMarkers([], []);
    this.setPath([...outline], ok ? PREVIEW_COLOR : PREVIEW_REFUSED_COLOR, null);
  }

  /** A transmission line: `ends` is `[ax, ay, aJoin, bx, by, bJoin]`, white markers where an
   * end joins; empty draws a refused red line from `a` to `b`. `null` ends hide it. */
  setLine(ends: Float32Array | null, a: Spot | null, b: Spot | null) {
    const markers: number[] = [];
    const colors: number[] = [];
    let line: number[] = [];
    let color = PREVIEW_COLOR;
    if (ends && ends.length === 6) {
      line = [ends[0], ends[1], ends[3], ends[4]];
      for (const i of [0, 3]) {
        if (ends[i + 2] !== 1) continue;
        markers.push(ends[i], 1, ends[i + 1]);
        colors.push(1, 1, 1);
      }
    } else if (ends && a && b) {
      color = PREVIEW_REFUSED_COLOR;
      line = [a.x, a.z, b.x, b.z];
    }
    this.setPath(line, color, null);
    this.setMarkers(markers, colors);
  }

  /** Outlines the rectangle the zone brush would paint from `a` to `b`; `null` hides it. */
  setZoneDrag(a: THREE.Vector3 | null, b: THREE.Vector3 | null, zone: number) {
    this.zoneDrag.visible = !!(a && b);
    if (!a || !b) return;
    this.setZoneLoop([a.x, a.z, b.x, a.z, b.x, b.z, a.x, b.z], zone);
  }

  /** Outlines the block the zone brush would fill: `[x, y]` per sample, closed; empty hides it. */
  setBlockOutline(outline: Float32Array, zone: number) {
    this.setZoneLoop([...outline], zone);
    this.zoneDrag.visible = outline.length > 4;
  }

  /** Glides markers toward their targets over `seconds`; `glide` false jumps straight there. */
  update(seconds: number, glide: boolean) {
    const position = this.markers.geometry.getAttribute('position') as
      | THREE.BufferAttribute
      | undefined;
    if (!position) return;
    const shown = position.array as Float32Array;
    const k = glide ? 1 - Math.exp(-seconds * GLIDE_RATE) : 1;
    let moving = false;
    for (let i = 0; i < shown.length; i++) {
      const gap = this.markerTarget[i] - shown[i];
      if (gap === 0) continue;
      shown[i] = Math.abs(gap) < 0.01 ? this.markerTarget[i] : shown[i] + gap * k;
      moving = true;
    }
    if (moving) position.needsUpdate = true;
  }

  /** `xy` is `[x, y]` per centreline point; `roadClass` null draws no ribbon. */
  private setPath(xy: number[], color: THREE.Color, roadClass: number | null) {
    const points: THREE.Vector3[] = [];
    const segments: number[] = [];
    for (let i = 0; i < xy.length; i += 2) {
      points.push(new THREE.Vector3(xy[i], LINE_Y, xy[i + 1]));
      if (i >= 2 && roadClass !== null)
        segments.push(xy[i - 2], xy[i - 1], xy[i], xy[i + 1], roadClass);
    }
    (this.line.material as THREE.LineBasicMaterial).color.copy(color);
    this.line.visible = points.length > 1;
    this.line.geometry.dispose();
    this.line.geometry = new THREE.BufferGeometry().setFromPoints(points);
    this.ribbon.material.color.copy(color);
    this.ribbon.visible = segments.length > 0;
    this.ribbon.geometry.dispose();
    this.ribbon.geometry = buildRoadGeometry(new Float32Array(segments));
  }

  /** Same marker count as now: keep drawn positions and glide. Otherwise jump. */
  private setMarkers(markers: number[], colors: number[]) {
    const target = new Float32Array(markers);
    const old = this.markers.geometry.getAttribute('position');
    const shown =
      old && old.array.length === target.length ? (old.array as Float32Array) : target.slice();
    this.markerTarget = target;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(shown, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    this.markers.geometry.dispose();
    this.markers.geometry = geom;
  }

  private setZoneLoop(xy: number[], zone: number) {
    const points = [];
    for (let i = 0; i < xy.length; i += 2) points.push(new THREE.Vector3(xy[i], LINE_Y, xy[i + 1]));
    this.zoneDrag.geometry.dispose();
    this.zoneDrag.geometry = new THREE.BufferGeometry().setFromPoints(points);
    (this.zoneDrag.material as THREE.LineBasicMaterial).color.set(ZONE_OUTLINE[zone] ?? 0xffffff);
  }
}
