// Three.js scene: roads as asphalt ribbons and pads, agents drawn by `AgentLayer`. Sim (x, y) maps
// to scene-local (x, z), then world (x, -z) with y up (see scene.ts).
//
// Current choices, all cheap to change:
// - WebGLRenderer. WebGPURenderer is still experimental; write new shaders in TSL so switching
//   later is easy.
// - camera-controls for the camera (MapControls was cruder at damping, bounds, zoom-to-cursor).
// - Agent matrices are written on the CPU each frame. If that becomes the bottleneck, move
//   interpolation and orientation into a vertex shader fed by the raw snapshot.
// - Picking is a ray against the ground plane. Add three-mesh-bvh once we pick against meshes.
// - Roads share one static mesh, rebuilt only when the road snapshot changes.
// - Curve sampling belongs to the sim.

import CameraControls from 'camera-controls';
import * as THREE from 'three';
import { AgentLayer } from './agent-layer';
import { buildBuildingGeometry } from './building-geometry';
import { followPoint, KeyCamera } from './camera';
import { carPosition, nearestCar } from './car-pick';
import { ensureInstances, standingBox } from './instances';
import { buildRoadGeometry } from './road-geometry';
import { createSimScene } from './scene';

CameraControls.install({ THREE });

const SIGNAL_STRIDE = 4;
/** Indexed by signal state: red, green, protected-left green. */
const SIGNAL_COLORS = [
  new THREE.Color(0xe5484d),
  new THREE.Color(0x46c46e),
  new THREE.Color(0x3fb8e8),
];
const SIGNAL_SIZE = 1.5;
const SIGNAL_HEIGHT = 4;
const BG = 0x0f1419;
const ASPHALT_COLOR = 0x3a4654;
const PREVIEW_COLOR = new THREE.Color(0x4a8bd0);
const PREVIEW_REFUSED_COLOR = new THREE.Color(0xe5484d);
/** Snap markers, indexed by join: new end (not drawn), existing node, split road. */
const JOIN_COLORS = [null, new THREE.Color(0xffffff), new THREE.Color(0xf5c542)];
/** The hover marker before a road has a start: new ends get one too, so the start is visible. */
const JOIN_MARKER = [PREVIEW_COLOR, JOIN_COLORS[1], JOIN_COLORS[2]];
const MARKER_PX = 10;
/**
 * Zone overlay colors, indexed by `Zone` value; low density light, high density dark. Mixed
 * with the background so the overlay can be opaque: nothing else is drawn under it.
 */
const ZONE_COLORS = [null, 0x7bd88f, 0x2e9e4f, 0x6fb3ff, 0x2f6fd0, 0xf2c94c, 0xd9822b].map((c) =>
  c === null ? null : new THREE.Color(BG).lerp(new THREE.Color(c), 0.45),
);
const LOT_COLOR = 0x9aa7b4;
/** Bright versions for the drag outline, which must stand out. */
const ZONE_OUTLINE = [0xe5484d, 0x7bd88f, 0x2e9e4f, 0x6fb3ff, 0x2f6fd0, 0xf2c94c, 0xd9822b];

export class Renderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = createSimScene();
  readonly camera: THREE.PerspectiveCamera;
  readonly controls: CameraControls;
  /** Held-key camera motion; main.ts reports the keys each frame. */
  readonly keyCamera: KeyCamera;
  private readonly clock = new THREE.Clock();
  private roads?: THREE.Mesh;
  private buildings?: THREE.Mesh;
  private readonly agents = new AgentLayer(this.scene);
  private signals?: THREE.InstancedMesh;
  private readonly preview: THREE.Line;
  private readonly markers: THREE.Points;
  private zones?: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
  private readonly zoneDrag: THREE.LineLoop;
  private lots?: THREE.LineSegments;
  private readonly matrix = new THREE.Matrix4();
  private readonly quat = new THREE.Quaternion();
  private readonly pos = new THREE.Vector3();
  private readonly scale = new THREE.Vector3(1, 1, 1);
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly look = new THREE.Vector3();
  /** Called when a mouse or touch drag pans the camera, so follow can let go. */
  onUserPan: (() => void) | null = null;

  constructor(container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.setSize(innerWidth, innerHeight);
    container.appendChild(this.renderer.domElement);
    this.scene.background = new THREE.Color(BG);

    this.camera = new THREE.PerspectiveCamera(45, innerWidth / innerHeight, 1, 50000);
    this.controls = new CameraControls(this.camera, this.renderer.domElement);
    // City-builder mapping: left pans, right orbits, wheel zooms toward the cursor.
    this.controls.mouseButtons.left = CameraControls.ACTION.TRUCK;
    this.controls.mouseButtons.right = CameraControls.ACTION.ROTATE;
    this.controls.dollyToCursor = true;
    this.controls.verticalDragToForward = true;
    this.controls.minPolarAngle = 0;
    this.controls.maxPolarAngle = THREE.MathUtils.degToRad(70);
    this.controls.minDistance = 30;
    this.keyCamera = new KeyCamera(this.controls);
    const pans =
      CameraControls.ACTION.TRUCK |
      CameraControls.ACTION.SCREEN_PAN |
      CameraControls.ACTION.TOUCH_TRUCK |
      CameraControls.ACTION.TOUCH_SCREEN_PAN |
      CameraControls.ACTION.TOUCH_DOLLY_TRUCK |
      CameraControls.ACTION.TOUCH_ZOOM_TRUCK;
    this.controls.addEventListener('control', () => {
      if (this.controls.currentAction & pans) this.onUserPan?.();
    });

    this.scene.add(new THREE.HemisphereLight(0xdde6ef, 0x20262d, 2.5));
    const sun = new THREE.DirectionalLight(0xffffff, 1.2);
    sun.position.set(1, 2, 1);
    this.scene.add(sun);

    this.preview = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]),
      new THREE.LineBasicMaterial({ color: PREVIEW_COLOR }),
    );
    this.preview.visible = false;
    this.preview.frustumCulled = false;
    this.scene.add(this.preview);
    this.markers = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({ size: MARKER_PX, sizeAttenuation: false, vertexColors: true }),
    );
    this.markers.frustumCulled = false;
    this.scene.add(this.markers);
    this.zoneDrag = new THREE.LineLoop(new THREE.BufferGeometry(), new THREE.LineBasicMaterial());
    this.zoneDrag.visible = false;
    this.zoneDrag.frustumCulled = false;
    this.scene.add(this.zoneDrag);

    addEventListener('resize', () => {
      this.camera.aspect = innerWidth / innerHeight;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(innerWidth, innerHeight);
    });
  }

  /** `segments` is `[ax, ay, bx, by, class]` per active rendered segment (many per curve), not per road ID. */
  setRoads(segments: Float32Array, bounds: [number, number, number, number], fitCamera: boolean) {
    const geom = buildRoadGeometry(segments);
    if (this.roads) {
      this.roads.geometry.dispose();
      this.roads.geometry = geom;
    } else {
      this.roads = new THREE.Mesh(geom, new THREE.MeshBasicMaterial({ color: ASPHALT_COLOR }));
      this.scene.add(this.roads);
    }

    if (fitCamera) {
      const [x0, y0, x1, y1] = bounds;
      const cx = (x0 + x1) / 2;
      const cz = -(y0 + y1) / 2;
      const span = Math.max(x1 - x0, y1 - y0);
      this.controls.setLookAt(cx, span * 0.9, cz + span * 0.5, cx, 0, cz, false);
      this.controls.maxDistance = span * 2;
      const pad = span * 0.25;
      this.controls.setBoundary(
        new THREE.Box3(
          new THREE.Vector3(x0 - pad, 0, -y1 - pad),
          new THREE.Vector3(x1 + pad, 0, -y0 + pad),
        ),
      );
    }
  }

  /** Sim-owned footprints and heights; rebuilt only when building geometry changes. */
  setBuildings(data: Float32Array) {
    const geometry = buildBuildingGeometry(data);
    if (this.buildings) {
      this.buildings.geometry.dispose();
      this.buildings.geometry = geometry;
    } else {
      this.buildings = new THREE.Mesh(
        geometry,
        new THREE.MeshLambertMaterial({ vertexColors: true }),
      );
      this.scene.add(this.buildings);
    }
  }

  /** `signals` is `[x, y, heading, state]` per signalised approach, at its stop line. */
  drawSignals(signals: Float32Array) {
    const count = signals.length / SIGNAL_STRIDE;
    const material = this.signals
      ? (this.signals.material as THREE.Material)
      : new THREE.MeshBasicMaterial();
    this.signals = ensureInstances(
      this.scene,
      this.signals,
      count,
      () => standingBox([SIGNAL_SIZE, SIGNAL_HEIGHT, SIGNAL_SIZE]),
      material,
    );
    this.quat.identity();
    for (let i = 0; i < count; i++) {
      const o = i * SIGNAL_STRIDE;
      this.pos.set(signals[o], 0, signals[o + 1]);
      this.matrix.compose(this.pos, this.quat, this.scale);
      this.signals.setMatrixAt(i, this.matrix);
      this.signals.setColorAt(i, SIGNAL_COLORS[signals[o + 3]] ?? SIGNAL_COLORS[0]);
    }
    this.signals.count = count;
    this.signals.instanceMatrix.needsUpdate = true;
    if (this.signals.instanceColor) this.signals.instanceColor.needsUpdate = true;
  }

  /** Draws cars and walkers between two snapshots; see `AgentLayer.draw`. */
  drawAgents(
    prev: Float32Array | undefined,
    curr: Float32Array,
    t: number,
    selected: number | null = null,
  ) {
    this.agents.draw(prev, curr, t, selected);
  }

  /** Left drag pans the camera, unless a tool needs the drag (the zone brush). */
  setLeftDragPans(pans: boolean) {
    this.controls.mouseButtons.left = pans
      ? CameraControls.ACTION.TRUCK
      : CameraControls.ACTION.NONE;
  }

  /** The agent id of the drawn car or walker nearest a screen point, within a few pixels, or null. */
  carAt(clientX: number, clientY: number): number | null {
    return nearestCar(
      this.agents.drawn,
      this.scene,
      this.camera,
      clientX,
      clientY,
      innerWidth,
      innerHeight,
    );
  }

  /** Eases the camera toward agent `agent`; holds still and returns false while it is hidden. */
  follow(agent: number, seconds: number): boolean {
    if (!carPosition(this.agents.drawn, this.scene, agent, this.look)) return false;
    followPoint(this.controls, this.look, seconds);
    return true;
  }

  /** While following, the wheel zooms toward the followed car instead of the cursor. */
  setFollowing(on: boolean) {
    this.controls.dollyToCursor = !on;
  }

  /** Ground-plane point in scene-local (sim x, y as x, z), or null if the ray misses. */
  pick(clientX: number, clientY: number): THREE.Vector3 | null {
    const ndc = new THREE.Vector2((clientX / innerWidth) * 2 - 1, -(clientY / innerHeight) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const point = ray.ray.intersectPlane(new THREE.Plane(this.up, 0), new THREE.Vector3());
    return point ? this.scene.worldToLocal(point) : null;
  }

  /**
   * Draws the road the sim would build from `a` to `b`. `stops` is `[x, y, join]` per stop
   * (see `roadPreview` in protocol.ts): the line follows the stops, with a marker where it joins
   * the network. Join -1 marks interior curve samples without a marker. Empty `stops` means
   * refused: a red straight line. A single stop is where a road would start, drawn as a marker
   * even for a new end. `null` hides the preview.
   */
  setPreview(stops: Float32Array | null, a: THREE.Vector3 | null, b: THREE.Vector3 | null) {
    const line: THREE.Vector3[] = [];
    const markers: number[] = [];
    const colors: number[] = [];
    const material = this.preview.material as THREE.LineBasicMaterial;
    if (stops && stops.length > 0) {
      material.color.copy(PREVIEW_COLOR);
      for (let i = 0; i < stops.length; i += 3) {
        line.push(new THREE.Vector3(stops[i], 0.5, stops[i + 1]));
        const c = stops.length === 3 ? JOIN_MARKER[stops[2]] : JOIN_COLORS[stops[i + 2]];
        if (!c) continue;
        markers.push(stops[i], 1, stops[i + 1]);
        colors.push(c.r, c.g, c.b);
      }
    } else if (stops && a && b) {
      material.color.copy(PREVIEW_REFUSED_COLOR);
      line.push(a.clone().setY(0.5), b.clone().setY(0.5));
    }
    this.preview.visible = line.length > 1;
    this.preview.geometry.dispose();
    this.preview.geometry = new THREE.BufferGeometry().setFromPoints(line);
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(markers, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    this.markers.geometry.dispose();
    this.markers.geometry = geom;
  }

  /**
   * Draws the zone grid as colored ground under the roads. `cells` is one `Zone` byte per cell,
   * row-major; `layout` is `[originX, originY, cellSize, cols, rows]` (see `zones` in protocol.ts).
   */
  setZones(cells: Uint8Array, layout: [number, number, number, number, number]) {
    const [x0, y0, size, cols, rows] = layout;
    const rgba = new Uint8Array(cols * rows * 4);
    for (let i = 0; i < cells.length; i++) {
      const c = ZONE_COLORS[cells[i]];
      if (!c) continue;
      rgba.set([c.r * 255, c.g * 255, c.b * 255, 255], i * 4);
    }
    // Row 0 of the texture is the grid's first row, at v = 0, which the geometry puts at y0.
    const texture = new THREE.DataTexture(rgba, cols, rows);
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.needsUpdate = true;
    if (!this.zones) {
      const [x1, y1] = [x0 + cols * size, y0 + rows * size];
      const geom = new THREE.BufferGeometry();
      geom.setAttribute(
        'position',
        new THREE.Float32BufferAttribute([x0, 0, y0, x1, 0, y0, x1, 0, y1, x0, 0, y1], 3),
      );
      geom.setAttribute('uv', new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2));
      geom.setIndex([0, 1, 2, 0, 2, 3]);
      // Unzoned texels are discarded. No depth write and drawn first, so roads cover it.
      const material = new THREE.MeshBasicMaterial({
        alphaTest: 0.5,
        depthWrite: false,
        side: THREE.DoubleSide,
      });
      this.zones = new THREE.Mesh(geom, material);
      this.zones.renderOrder = -1;
      this.zones.frustumCulled = false;
      this.scene.add(this.zones);
    }
    this.zones.material.map?.dispose();
    this.zones.material.map = texture;
    this.zones.material.needsUpdate = true;
  }

  /** Outlines the rectangle the zone brush would paint from `a` to `b`; `null` hides it. */
  setZoneDrag(a: THREE.Vector3 | null, b: THREE.Vector3 | null, zone: number) {
    this.zoneDrag.visible = !!(a && b);
    if (!a || !b) return;
    const corners = [a.x, a.z, b.x, a.z, b.x, b.z, a.x, b.z];
    const points = [];
    for (let i = 0; i < corners.length; i += 2)
      points.push(new THREE.Vector3(corners[i], 0.5, corners[i + 1]));
    this.zoneDrag.geometry.dispose();
    this.zoneDrag.geometry = new THREE.BufferGeometry().setFromPoints(points);
    (this.zoneDrag.material as THREE.LineBasicMaterial).color.set(ZONE_OUTLINE[zone] ?? 0xffffff);
  }

  /** Outlines the block the zone brush would fill: `[x, y]` per sample, closed; empty hides it. */
  setBlockOutline(outline: Float32Array, zone: number) {
    const points = [];
    for (let i = 0; i < outline.length; i += 2)
      points.push(new THREE.Vector3(outline[i], 0.5, outline[i + 1]));
    this.zoneDrag.visible = points.length > 2;
    this.zoneDrag.geometry.dispose();
    this.zoneDrag.geometry = new THREE.BufferGeometry().setFromPoints(points);
    (this.zoneDrag.material as THREE.LineBasicMaterial).color.set(ZONE_OUTLINE[zone] ?? 0xffffff);
  }

  /** Draws lot edges over the zones: `[n, x0, y0, …]` per lot (see `lots` in protocol.ts). */
  setLots(rings: Float32Array) {
    const pos: number[] = [];
    for (let i = 0; i < rings.length; ) {
      const n = rings[i];
      const at = (k: number) => i + 1 + 2 * (k % n);
      for (let k = 0; k < n; k++) {
        const [a, b] = [at(k), at(k + 1)];
        pos.push(rings[a], 0.1, rings[a + 1], rings[b], 0.1, rings[b + 1]);
      }
      i += 1 + 2 * n;
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(pos), 3));
    if (!this.lots) {
      // Drawn just after the zones and before the roads, which cover lot edges under them.
      const material = new THREE.LineBasicMaterial({
        color: LOT_COLOR,
        transparent: true,
        opacity: 0.35,
        depthWrite: false,
      });
      this.lots = new THREE.LineSegments(geom, material);
      this.lots.renderOrder = -1;
      this.lots.frustumCulled = false;
      this.scene.add(this.lots);
    } else {
      this.lots.geometry.dispose();
      this.lots.geometry = geom;
    }
  }

  /** Draw the full centreline selected by the sim, in the destructive-action color. */
  setBulldozePreview(segment: Float32Array) {
    this.setPreview(null, null, null);
    const line: THREE.Vector3[] = [];
    for (let i = 0; i < segment.length; i += 2) {
      line.push(new THREE.Vector3(segment[i], 0.5, segment[i + 1]));
    }
    (this.preview.material as THREE.LineBasicMaterial).color.copy(PREVIEW_REFUSED_COLOR);
    this.preview.geometry.dispose();
    this.preview.geometry = new THREE.BufferGeometry().setFromPoints(line);
    this.preview.visible = line.length > 1;
  }

  /** Draws a frame; true if the camera moved. */
  render(): boolean {
    const moved = this.controls.update(this.clock.getDelta());
    this.renderer.render(this.scene, this.camera);
    return moved;
  }
}
