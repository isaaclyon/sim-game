// What a tool would do, drawn over the city: a road-wide translucent ribbon along its centre
// line, snap markers, and the zone brush outline. The sim decides every shape; this module only
// draws its answers, and lets snap markers glide between answers instead of jumping.
import * as THREE from 'three';
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
const LINE_Y = 0.5;
const RIBBON_Y = 0.3;
const RIBBON_OPACITY = 0.35;

export class ToolPreview {
  readonly group = new THREE.Group();
  readonly line: THREE.Line;
  readonly ribbon: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  readonly markers: THREE.Points;
  readonly zoneDrag: THREE.LineLoop;
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
    for (const o of [this.line, this.ribbon, this.markers, this.zoneDrag]) {
      o.frustumCulled = false;
      this.group.add(o);
    }
  }

  /** See `Renderer.setPreview`. `roadClass` sets the ribbon width. */
  setRoad(
    stops: Float32Array | null,
    a: THREE.Vector3 | null,
    b: THREE.Vector3 | null,
    roadClass = 0,
  ) {
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

  /** The full centreline the bulldozer would remove, `[x, y]` per sample, in the refusal red. */
  setBulldoze(segment: Float32Array) {
    this.setMarkers([], []);
    this.setPath([...segment], PREVIEW_REFUSED_COLOR, 0);
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
