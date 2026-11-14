// Three.js scene: roads as flat ribbons and pads (`RoadSurface`), agents drawn by `AgentLayer`. Sim (x, y) maps
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
// - Ground, water, fields and trees are decoration from `scenery/`, never sim state. So are
//   streetlights (`streetlights.ts`), placed along the roads snapshot and lit at night.
// - Flat layers share y = 0 without depth writes. Ground draws first; the zone overlay, pollution
//   or services, lots, sidewalks, roads, lane paint and the traffic view draw in the transparent
//   pass, after buildings and cars, in that order.
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
import { DataOverlays } from './data-overlays.ts';
import { FireLayer } from './fire-layer.ts';
import { compactMaterial, Lighting, nightMaterial } from './lighting.ts';
import { LotLayer } from './lot-scenery/layer.ts';
import { SKY } from './palette';
import { PowerLayer } from './power-layer.ts';
import type { Spot } from './power-tools.ts';
import type { Era, FromWorker } from './protocol.ts';
import { RoadSurface } from './road-surface.ts';
import { createSimScene } from './scene';
import { Scenery } from './scenery/index.ts';
import { SchoolLayer } from './school-layer.ts';
import { SignalLayer } from './signal-layer.ts';
import { SmokeLayer } from './smoke-layer.ts';
import { StreetlightLayer } from './streetlights.ts';
import { type Ground, ToolPreview } from './tool-preview.ts';
import type { Picture } from './ui/catalog.ts';
import { drawPictures } from './ui/pictures.ts';
import type { PollutionView } from './ui/views.ts';
import { WaterLayer } from './water-layer.ts';
import { ZoneOverlay } from './zone-overlay';

CameraControls.install({ THREE });

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
  private readonly roads = new RoadSurface();
  private readonly lighting = new Lighting(this.scene);
  private readonly buildings = new BuildingLayer(
    compactMaterial(nightMaterial(this.lighting.night, this.lighting.lightHour)),
  );
  private readonly scenery = new Scenery();
  private readonly lotScenery = new LotLayer();
  private readonly overlay = new ZoneOverlay();
  /** Pollution, traffic and services views (ADR 0037, 0057); main.ts feeds the latter two. */
  readonly ground = new DataOverlays();
  private readonly smoke = new SmokeLayer(this.lighting.night);
  private readonly agents = new AgentLayer(this.scene, nightMaterial(this.lighting.night));
  private readonly signals = new SignalLayer(this.scene);
  private readonly streetlights = new StreetlightLayer(nightMaterial(this.lighting.streetlights));
  private readonly tools = new ToolPreview();
  private readonly power = new PowerLayer();
  private readonly water = new WaterLayer();
  private readonly fire = new FireLayer();
  private readonly schools = new SchoolLayer();
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
    // Building and lot tiles keep their vertices only on the GPU; rebuild them if it loses them.
    this.renderer.domElement.addEventListener('webglcontextrestored', () => {
      this.buildings.restore();
      this.lotScenery.restore();
    });
    this.scene.add(
      this.scenery.group,
      this.roads.group,
      this.lotScenery.group,
      this.overlay.group,
      this.ground.group,
      this.buildings.group,
      this.power.group,
      this.water.group,
      this.fire.group,
      this.schools.group,
      this.smoke.group,
      this.streetlights.group,
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
    this.roads.set(segments);
    this.scenery.setRoads(segments);
    this.lotScenery.setRoads(segments);
    this.streetlights.set(segments);

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

  /** Plain shows haze and stains; the others tint the ground, roads or lots by one measure. */
  setPollutionView(view: PollutionView) {
    this.ground.setView(view);
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

  /** Moves the camera, smoothly, to look at sim point (x, y). */
  showPoint(x: number, y: number) {
    this.look.set(x, 0, y);
    this.scene.localToWorld(this.look);
    void this.controls.moveTo(this.look.x, this.look.y, this.look.z, true);
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

  /** Ground metres one screen pixel spans at the point, across or down, whichever is more;
   * 0 where the view misses the ground. */
  metresPerPixel(clientX: number, clientY: number): number {
    const p = this.pick(clientX, clientY);
    const right = this.pick(clientX + 1, clientY);
    const down = this.pick(clientX, clientY + 1);
    return p && right && down ? Math.max(p.distanceTo(right), p.distanceTo(down)) : 0;
  }

  /**
   * Draws the road the sim would build from `a` to `b`. `stops` is `[x, y, join]` per stop
   * (see `roadPreview` in protocol.ts): the line follows the stops, with a marker where it joins
   * the network. Join -1 marks interior curve samples without a marker. Empty `stops` means
   * refused: a red straight line. A single stop is where a road would start, drawn as a marker
   * even for a new end. `null` hides the preview. `reclass` is `[n, x0, y0, …]` per existing
   * road the build would change to `roadClass` (ADR 0048), highlighted at that class's width.
   */
  setPreview(
    stops: Float32Array | null,
    a: Ground | null,
    b: Ground | null,
    roadClass = 0,
    reclass = new Float32Array(),
  ) {
    this.tools.setRoad(stops, a, b, roadClass, reclass);
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

  /** The road an upgrade click would change, at its new class's width (ADR 0050). */
  setUpgradePreview(segment: Float32Array, ok: boolean, roadClass: number) {
    this.tools.setUpgrade(segment, ok, roadClass);
  }

  /** The grid a click would build; see `ToolPreview.setGrid` (ADR 0049). */
  setGridPreview(
    roads: Float32Array,
    dropped: Float32Array,
    edge: Float32Array,
    ok: boolean,
    roadClass: number,
  ) {
    this.tools.setGrid(roads, dropped, edge, ok, roadClass);
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

  /** Draws pumps, towers and water markers; see `water` in protocol.ts. */
  setWater(pieces: Float32Array, buildings: Float32Array) {
    this.water.set(pieces, buildings);
  }

  /** The water view marks dense buildings, pumps and towers with their network's colour. */
  setWaterView(on: boolean) {
    this.water.setView(on);
  }

  /** Draws stations, fires, ruins and coverage; see `fire` in protocol.ts. */
  setFire(fire: Extract<FromWorker, { type: 'fire' }>) {
    this.fire.set(fire);
  }

  /** Draws the schools; see `schools` in protocol-services.ts. */
  setSchools(schools: Float32Array) {
    this.schools.set(schools);
  }

  /** The fire view shows the roads and buildings stations reach (ADR 0053). */
  setFireView(on: boolean) {
    this.fire.setView(on);
  }

  /** See `ToolPreview.setHistory`; empty lists hide it (ADR 0051). */
  setHistoryPreview(remove: Float32Array, restore: Float32Array, ok: boolean) {
    this.tools.setHistory(remove, restore, ok);
  }

  /** See `ToolPreview.setPlant`. */
  setPlantPreview(outline: Float32Array, ok: boolean) {
    this.tools.setPlant(outline, ok);
  }

  /** See `ToolPreview.setLine`; `null` ends hide the preview. */
  setLinePreview(ends: Float32Array | null, a: Spot | null, b: Spot | null) {
    this.tools.setLine(ends, a, b);
  }

  /** The build menu's pictures, drawn once from the game's models: data URLs by `pictureKey`. */
  drawPictures(pictures: Picture[]): Map<string, string> {
    return drawPictures(this.renderer, pictures);
  }

  /** The city's era from frame stats: dirt village roads or paved town ones (ADR 0059). */
  setEra(era: Era) {
    this.roads.setEra(era);
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
    this.fire.animate(reducedMotion() ? 0 : now);
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
    this.streetlights.update(this.lighting.streetlights.value);
    this.renderer.render(this.scene, this.camera);
    return moved;
  }
}
