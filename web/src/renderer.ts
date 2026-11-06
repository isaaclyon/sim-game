// Three.js scene: roads as asphalt ribbons and pads, agents drawn by `AgentLayer`. Sim (x, y) maps
// to scene-local (x, z), then world (x, -z) with y up (see scene.ts).
//
// Current choices, all cheap to change:
// - WebGLRenderer. WebGPURenderer is still experimental; write new shaders in TSL so switching
//   later is easy.
// - camera-controls for the camera (MapControls was cruder at damping, bounds, zoom-to-cursor).
// - Agent matrices are written on the CPU each frame. If that becomes the bottleneck, move
//   interpolation and orientation into a vertex shader fed by the raw snapshot.
// - Tools pick against the ground plane; building inspection picks roof and facade triangles.
// - Roads share one static mesh, rebuilt only when the road snapshot changes.
// - Ground, water, fields and trees are decoration from `scenery/`, never sim state.
// - Flat layers share y = 0 without depth writes. Ground draws first; the zone overlay, pollution, lots,
//   sidewalks, roads and lane paint draw in the transparent pass, after buildings and cars, in that order.
//   Chimney smoke draws after them (`SMOKE_ORDER`), so roads never show through it.
// - Curve sampling belongs to the sim. Tool previews (`tool-preview.ts`) draw the sim's answers.
// - Rising buildings, zone paint flashes and gliding markers are display-only motion, skipped
//   when the system asks for reduced motion.

import CameraControls from 'camera-controls';
import * as THREE from 'three';
import { AgentLayer } from './agent-layer';
import { pixelsPerMetre } from './agent-lod.ts';
import type { Listener } from './ambience';
import { BuildingLayer } from './building-kit/layer.ts';
import { followPoint, KeyCamera } from './camera';
import { carPosition, nearestCar } from './car-pick';
import { Lighting, nightMaterial } from './lighting.ts';
import { LotLayer } from './lot-scenery/layer.ts';
import { ROADS, SIDEWALK, SKY } from './palette';
import { PollutionOverlay } from './pollution-overlay.ts';
import { PowerLayer } from './power-layer.ts';
import type { Spot } from './power-tools.ts';
import { buildRoadGeometry } from './road-geometry';
import { buildRoadMarkings } from './road-markings.ts';
import { createSimScene } from './scene';
import { Scenery } from './scenery/index.ts';
import { SignalLayer } from './signal-layer.ts';
import { SmokeLayer } from './smoke-layer.ts';
import { type Ground, ToolPreview } from './tool-preview.ts';
import type { PollutionView } from './view-buttons.ts';
import { ZoneOverlay } from './zone-overlay';

CameraControls.install({ THREE });

const ROAD_COLORS = ROADS.map((c) => new THREE.Color(c));
const SIDEWALK_WIDTH = 2;
/** Rising buildings, paint flashes and gliding markers skip their motion when asked. */
const reducedMotion = (): boolean =>
  globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
const seconds = () => performance.now() / 1000;

export class Renderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = createSimScene();
  readonly camera: THREE.PerspectiveCamera;
  readonly controls: CameraControls;
  /** Held-key camera motion; main.ts reports the keys each frame. */
  readonly keyCamera: KeyCamera;
  private readonly clock = new THREE.Clock();
  private roads?: THREE.Mesh;
  private sidewalks?: THREE.Mesh;
  private markings?: THREE.Mesh;
  private readonly lighting = new Lighting(this.scene);
  private readonly buildings = new BuildingLayer(
    nightMaterial(this.lighting.night, this.lighting.lightHour),
  );
  private readonly scenery = new Scenery();
  private readonly lotScenery = new LotLayer();
  private readonly overlay = new ZoneOverlay();
  private readonly pollution = new PollutionOverlay();
  private readonly smoke = new SmokeLayer(this.lighting.night);
  private readonly agents = new AgentLayer(this.scene, nightMaterial(this.lighting.night));
  private readonly signals = new SignalLayer(this.scene);
  private readonly tools = new ToolPreview();
  private readonly power = new PowerLayer();
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly look = new THREE.Vector3();
  /** Called when a mouse or touch drag pans the camera, so follow can let go. */
  onUserPan: (() => void) | null = null;

  constructor(container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.setSize(innerWidth, innerHeight);
    container.appendChild(this.renderer.domElement);
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.scene.add(
      this.scenery.group,
      this.lotScenery.group,
      this.overlay.group,
      this.pollution.group,
      this.buildings.group,
      this.power.group,
      this.smoke.group,
      this.tools.group,
    );

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

    addEventListener('resize', () => {
      this.camera.aspect = innerWidth / innerHeight;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(innerWidth, innerHeight);
    });
  }

  /** `segments` is `[ax, ay, bx, by, class]` per active rendered segment (many per curve), not per road ID. */
  setRoads(segments: Float32Array, bounds: [number, number, number, number], fitCamera: boolean) {
    this.sidewalks = this.flatLayer(
      this.sidewalks,
      buildRoadGeometry(segments, SIDEWALK_WIDTH),
      new THREE.MeshLambertMaterial({ color: SIDEWALK }),
      3,
    );
    this.roads = this.flatLayer(
      this.roads,
      buildRoadGeometry(segments, 0, ROAD_COLORS),
      new THREE.MeshLambertMaterial({ vertexColors: true }),
      4,
    );
    this.markings = this.flatLayer(
      this.markings,
      buildRoadMarkings(segments),
      new THREE.MeshLambertMaterial({ vertexColors: true }),
      5,
    );
    this.scenery.setRoads(segments);
    this.lotScenery.setRoads(segments);

    if (fitCamera) {
      const [x0, y0, x1, y1] = bounds;
      const cx = (x0 + x1) / 2;
      const cz = -(y0 + y1) / 2;
      const span = Math.max(x1 - x0, y1 - y0);
      // Land reaches past the pan boundary and fades into the sky before its edge.
      const land = span * 0.35;
      this.scenery.setExtent([x0 - land, y0 - land, x1 + land, y1 + land]);
      this.scene.fog = new THREE.Fog(SKY, span * 1.3, span * 3);
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

  /** Swaps `geometry` into `mesh`, or makes a flat transparent-pass mesh at `order`. */
  private flatLayer(
    mesh: THREE.Mesh | undefined,
    geometry: THREE.BufferGeometry,
    material: THREE.Material,
    order: number,
  ): THREE.Mesh {
    if (mesh) {
      mesh.geometry.dispose();
      mesh.geometry = geometry;
      return mesh;
    }
    Object.assign(material, { transparent: true, depthWrite: false });
    const created = new THREE.Mesh(geometry, material);
    created.receiveShadow = true;
    created.renderOrder = order;
    created.frustumCulled = false;
    this.scene.add(created);
    return created;
  }

  /**
   * Sim-owned footprints, heights and fronts; only changed tiles are rebuilt. With `rise`, new
   * building IDs rise from the ground; a city's first snapshot should appear at once.
   */
  setBuildings(data: Float32Array, ids: Uint32Array, rise = false) {
    const from = rise && !reducedMotion() ? seconds() : null;
    this.buildings.update(data, ids, from);
    this.smoke.update(data, ids, from);
    this.lotScenery.setBuildings(data);
  }

  /** `signals` is `[x, y, heading, state]` per signalised approach; see `SignalLayer`. */
  drawSignals(signals: Float32Array) {
    this.signals.draw(signals);
  }

  /** Draws cars and walkers between two snapshots; see `AgentLayer.draw`. */
  drawAgents(
    prev: Float32Array | undefined,
    curr: Float32Array,
    t: number,
    selected: number | null = null,
  ) {
    this.agents.draw(
      prev,
      curr,
      t,
      selected,
      this.camera,
      pixelsPerMetre(this.camera, innerHeight),
    );
  }

  /** Switches cars between paint and speed colours; returns whether traffic view is on. */
  toggleTrafficView(): boolean {
    this.agents.trafficView = !this.agents.trafficView;
    return this.agents.trafficView;
  }

  /** `cells` and `layout` as in the `pollution` message (ADR 0037). */
  setPollution(cells: Uint8Array, layout: [number, number, number, number, number]) {
    this.pollution.setCells(cells, layout);
  }

  /** Plain shows faint haze and stains; the others tint the ground by one measure. */
  setPollutionView(view: PollutionView) {
    this.pollution.setView(view);
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

  /** Stable ID of the nearest drawn building under this screen point, or null. */
  buildingAt(clientX: number, clientY: number): number | null {
    const ray = new THREE.Raycaster();
    ray.setFromCamera(
      new THREE.Vector2((clientX / innerWidth) * 2 - 1, -(clientY / innerHeight) * 2 + 1),
      this.camera,
    );
    return this.buildings.pick(ray);
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

  /** Where ambient sound is heard: the sim point the camera looks at, and its distance. */
  listener(): Listener {
    this.scene.worldToLocal(this.controls.getTarget(this.look, false));
    return { x: this.look.x, y: this.look.z, distance: this.controls.distance };
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
  setPreview(stops: Float32Array | null, a: Ground | null, b: Ground | null, roadClass = 0) {
    this.tools.setRoad(stops, a, b, roadClass);
  }

  /** Dashed drawing guides, `[x0, y0, x1, y1]` per line (ADR 0038); null hides them. */
  setGuides(lines: Float32Array | null) {
    this.tools.setGuides(lines);
  }

  /** Angle marks where a drawn road joins roads, `[x, y, rx, ry, dx, dy]` each; null hides. */
  setAngles(marks: Float32Array | null) {
    this.tools.setAngles(marks);
  }

  /**
   * Tints zoned ground. `cells` is one `Zone` byte per cell, row-major; `layout` is
   * `[originX, originY, cellSize, cols, rows]` (see `zones` in protocol.ts). With `flash`,
   * cells that changed since the last grid briefly glow.
   */
  setZones(cells: Uint8Array, layout: [number, number, number, number, number], flash = false) {
    this.overlay.setZones(cells, layout, flash && !reducedMotion() ? seconds() : null);
    this.scenery.setZones(cells, layout);
  }

  /** Colors and property outlines are editing aids, not permanent scenery. */
  setZoningVisible(visible: boolean) {
    this.overlay.group.visible = visible;
  }

  /** Outlines the rectangle the zone brush would paint from `a` to `b`; `null` hides it. */
  setZoneDrag(a: THREE.Vector3 | null, b: THREE.Vector3 | null, zone: number) {
    this.tools.setZoneDrag(a, b, zone);
  }

  /** Outlines the block the zone brush would fill: `[x, y]` per sample, closed; empty hides it. */
  setBlockOutline(outline: Float32Array, zone: number) {
    this.tools.setBlockOutline(outline, zone);
  }

  /** Draws lot edges over the zones: `[n, x0, y0, …]` per lot (see `lots` in protocol.ts). */
  setLots(rings: Float32Array) {
    this.overlay.setLots(rings);
    this.scenery.setLots(rings);
    this.lotScenery.setLots(rings);
  }

  /** Draw the full centreline selected by the sim, in the destructive-action color. */
  setBulldozePreview(segment: Float32Array) {
    this.tools.setBulldoze(segment);
  }

  /** Draws plants, lines and power markers; see `power` in protocol.ts. */
  setPower(plants: Float32Array, lines: Float32Array, buildings: Float32Array) {
    this.power.set(plants, lines, buildings);
    this.smoke.setPlants(plants);
  }

  /** The power view marks every building with its grid's colour, not only unpowered ones. */
  setPowerView(on: boolean) {
    this.power.setView(on);
  }

  /** See `ToolPreview.setPlant`. */
  setPlantPreview(outline: Float32Array, ok: boolean) {
    this.tools.setPlant(outline, ok);
  }

  /** See `ToolPreview.setLine`; `null` ends hide the preview. */
  setLinePreview(ends: Float32Array | null, a: Spot | null, b: Spot | null) {
    this.tools.setLine(ends, a, b);
  }

  /** Snapshot time only: paused frames keep the same lighting. */
  setTime(secondOfDay: number) {
    this.lighting.setTime(secondOfDay);
  }

  /** Draws a frame; true if the camera moved. */
  render(): boolean {
    const delta = this.clock.getDelta();
    const moved = this.controls.update(delta);
    const now = seconds();
    this.buildings.animate(now);
    this.overlay.animate(now);
    this.power.animate(now);
    // Reduced motion holds the smoke still, at a moment when every chimney has puffs out.
    const target = this.scene.worldToLocal(this.controls.getTarget(this.look, false));
    this.smoke.animate(reducedMotion() ? 1000 : now, [target.x, target.z]);
    this.tools.update(delta, !reducedMotion());
    this.scenery.update();
    this.lotScenery.update();
    this.lighting.update(
      this.scene,
      this.controls.getTarget(this.look, false),
      this.controls.distance,
    );
    this.renderer.render(this.scene, this.camera);
    return moved;
  }
}
