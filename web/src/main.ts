import type * as THREE from 'three';
import { AmbientSound } from './ambient-sound';
import { CityControls } from './city-controls';
import { drawDemand } from './demand-meter';
import { hudText } from './hud';
import { Inspector } from './inspector';
import { type FromWorker, RoadClass, Snap, type Stats, type ToWorker, Zone } from './protocol';
import { Renderer } from './renderer';
import { type RoadMode, RoadTool } from './road-tool';
import { bindViewButtons } from './view-buttons';

const byId = (id: string): HTMLElement => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing #${id} in index.html`);
  return el;
};

const renderer = new Renderer(document.body);
const sound = new AmbientSound(byId('sound') as HTMLButtonElement);
const hud = byId('hud');
const demand = byId('demand');
const demandFills = ['residential', 'commercial', 'industrial'].map((u) => byId(`demand-${u}`));
const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' });
const send = (msg: ToWorker) => worker.postMessage(msg);
const cityControls = new CityControls(send, byId, import.meta.env.DEV);
// Pan keys held when follow starts are ignored until released, so they cannot fight it.
const ignoredCameraKeys = new Set<string>();
const inspector = new Inspector(
  send,
  {
    follow: (agent, seconds) => renderer.follow(agent, seconds),
    setFollowing(on) {
      renderer.setFollowing(on);
      if (!on) return;
      renderer.keyCamera.stopPan();
      for (const key of 'wasd') if (heldCameraKeys.delete(key)) ignoredCameraKeys.add(key);
    },
  },
  byId('inspector'),
  byId('inspector-text'),
  byId('follow') as HTMLButtonElement,
  byId('inspector-close') as HTMLButtonElement,
);
renderer.onUserPan = () => inspector.setFollow(false);
const views = bindViewButtons(byId, renderer);

let prev: Float32Array | undefined;
let curr: Float32Array | undefined;
let currAt = 0;
let frameGap = 100;
let stats: Stats | undefined;
let tickMs = 0;
let roadsSeen = false;
// A city's first zones and buildings appear at once; later changes flash and rise.
let zonesSeen = false;
let buildingsSeen = false;

worker.onmessage = (e: MessageEvent<FromWorker>) => {
  const msg = e.data;
  if (msg.type === 'cityStatus') {
    cityControls.receive(msg);
    hud.hidden = !msg.ready;
  } else if (msg.type === 'cityFile' || msg.type === 'commandLog') {
    void cityControls.receiveFile(msg);
  } else if (msg.type === 'cityReset') {
    setTool(null);
    inspector.select(null);
    stopKeyCamera();
    pointer = null;
    blockInFlight = false;
    roads.reset();
    prev = curr = undefined;
    stats = undefined;
    roadsSeen = zonesSeen = buildingsSeen = false;
    frameGap = 100;
    setSpeed(msg.speed);
  } else if (msg.type === 'roads') {
    renderer.setRoads(msg.segments, msg.bounds, !roadsSeen);
    roadsSeen = true;
    // Blocks change with roads, including alleys the sim adds after zoning.
    if (blockHover) {
      blockWanted = true;
      sendBlockPreview();
    }
    // Retry a hover sent before WASM was ready, or after the topology changed.
    roads.roadsChanged();
  } else if (
    msg.type === 'roadPreview' ||
    msg.type === 'bulldozePreview' ||
    msg.type === 'buildResult'
  ) {
    roads.receive(msg);
  } else if (msg.type === 'zones') {
    renderer.setZones(msg.cells, msg.layout, zonesSeen);
    zonesSeen = true;
  } else if (msg.type === 'pollution') {
    renderer.setPollution(msg.cells, msg.layout);
  } else if (msg.type === 'lots') {
    renderer.setLots(msg.rings);
  } else if (msg.type === 'buildings') {
    renderer.setBuildings(msg.data, msg.ids, buildingsSeen);
    buildingsSeen = true;
    sound.setBuildings(msg.data);
  } else if (msg.type === 'citizen') {
    inspector.receive(msg.selection, msg.citizen);
  } else if (msg.type === 'building') {
    inspector.receiveBuilding(msg.selection, msg.building);
  } else if (msg.type === 'blockPreview') {
    blockInFlight = false;
    if (msg.revision === blockRevision && zone !== null && !zoneStart && blockHover)
      renderer.setBlockOutline(msg.outline, zone);
    sendBlockPreview();
  } else {
    const now = performance.now();
    if (curr) frameGap = frameGap * 0.8 + (now - currAt) * 0.2;
    prev = curr;
    curr = msg.agents;
    currAt = now;
    stats = msg.stats;
    tickMs = msg.tickMs;
    renderer.setTime(stats.secondOfDay);
    renderer.drawSignals(msg.signals);
  }
};

send({ type: 'init', seed: 1, cols: 60, rows: 60, agents: 0, grid: false });

let speed = 1;
const speedButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-speed]')];
function setSpeed(s: number) {
  speed = s;
  send({ type: 'setSpeed', speed: s });
  for (const b of speedButtons) b.classList.toggle('active', Number(b.dataset.speed) === s);
}
for (const b of speedButtons) b.onclick = () => setSpeed(Number(b.dataset.speed));

const roadButton = byId('road');
const curveButton = byId('curve');
// Main road (M) is a setting of both road tools, kept until switched off.
const mainRoadButton = byId('main-road');
const bulldozeButton = byId('bulldoze');
const roads = new RoadTool(send, renderer, {
  help: byId('road-help'),
  readout: byId('road-readout'),
  snapOptions: byId('snap-options'),
  snapButtons: [
    [byId('snap-angle'), Snap.Angle],
    [byId('snap-guides'), Snap.Guides],
    [byId('snap-length'), Snap.Length],
  ],
});
const zoneButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-zone]')];
let zone: Zone | null = null;
let zoneStart: THREE.Vector3 | null = null;
// The block under the cursor while the zone brush hovers, asked of the sim like road previews:
// one question in flight, about the latest position only.
let blockHover: THREE.Vector3 | null = null;
let blockWanted = false;
let blockInFlight = false;
let blockRevision = 0;
function sendBlockPreview() {
  if (!blockWanted || blockInFlight || !blockHover || zone === null || zoneStart) return;
  blockWanted = false;
  blockInFlight = true;
  send({ type: 'previewBlock', x: blockHover.x, y: blockHover.z, revision: blockRevision });
}
function hideBlockOutline() {
  blockRevision++;
  blockHover = null;
  blockWanted = false;
  renderer.setZoneDrag(null, null, 0);
}
function setTool(tool: RoadMode, brush: Zone | null = null) {
  zone = tool ? null : brush;
  zoneStart = null;
  hideBlockOutline();
  renderer.setLeftDragPans(zone === null);
  renderer.setZoningVisible(zone !== null);
  for (const b of zoneButtons) {
    const on = Number(b.dataset.zone) === zone;
    b.classList.toggle('active', on);
    b.setAttribute('aria-pressed', String(on));
  }
  roads.setMode(tool);
  for (const [button, mode] of [
    [roadButton, 'straight'],
    [curveButton, 'curve'],
    [bulldozeButton, 'bulldoze'],
  ] as const) {
    button.classList.toggle('active', tool === mode);
    button.setAttribute('aria-pressed', String(tool === mode));
  }
  renderer.renderer.domElement.style.cursor = tool || zone !== null ? 'crosshair' : '';
}
/** Toggles main road for the road tools, picking the straight road tool if none is active. */
function toggleMainRoad() {
  const on = roads.roadClass !== RoadClass.Main;
  roads.setRoadClass(on ? RoadClass.Main : RoadClass.Street);
  mainRoadButton.classList.toggle('active', on);
  mainRoadButton.setAttribute('aria-pressed', String(on));
  if (!roads.roadMode) setTool('straight');
}
function setZone(z: Zone) {
  setTool(null, zone === z ? null : z);
}
for (const b of zoneButtons) b.onclick = () => setZone(Number(b.dataset.zone) as Zone);
/** Z, X and C pick residential, commercial and industrial; pressing again switches density. */
function zoneKey(low: Zone, high: Zone) {
  setTool(null, zone === low ? high : low);
}
/** Picks a tool, or switches it off if it is already on. */
const toggleTool = (mode: RoadMode) => setTool(roads.mode === mode ? null : mode);
roadButton.onclick = () => toggleTool('straight');
curveButton.onclick = () => toggleTool('curve');
bulldozeButton.onclick = () => toggleTool('bulldoze');
mainRoadButton.onclick = toggleMainRoad;

const canvas = renderer.renderer.domElement;
let downX = 0;
let downY = 0;
// Keyboard camera motion during a zone press moves the ground under the cursor: a drag.
let zoneKeyPanned = false;
canvas.addEventListener('pointerdown', (e) => {
  downX = e.clientX;
  downY = e.clientY;
  if (zone === null || e.button !== 0) return;
  zoneStart = renderer.pick(e.clientX, e.clientY);
  zoneKeyPanned = false;
  blockRevision++;
  // Keep getting moves and the release even if the drag leaves the canvas.
  canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener('pointerup', (e) => {
  if (zone !== null) {
    if (e.button === 2) return setTool(null);
    const end = renderer.pick(e.clientX, e.clientY);
    if (e.button === 0 && zoneStart && end) {
      const [a, b] = [zoneStart, end];
      // A click fills the block under it; a drag paints the rectangle.
      if (!zoneKeyPanned && Math.hypot(e.clientX - downX, e.clientY - downY) <= 4)
        send({ type: 'fillBlock', x: b.x, y: b.z, zone });
      else send({ type: 'paintZone', ax: a.x, ay: a.z, bx: b.x, by: b.z, zone });
    }
    zoneStart = null;
    renderer.setZoneDrag(null, null, 0);
    // The outline comes back on the next move, or when the edit's roads arrive.
    blockHover = end;
    blockWanted = true;
    return;
  }
  // Treat it as a click only if the pointer barely moved, so panning still works.
  const still = Math.hypot(e.clientX - downX, e.clientY - downY) <= 4;
  if (!roads.mode) {
    if (e.button === 0 && still) {
      const agent = renderer.carAt(e.clientX, e.clientY);
      const building = agent === null ? renderer.buildingAt(e.clientX, e.clientY) : null;
      if (building !== null) inspector.selectBuilding(building);
      else inspector.select(agent);
    }
    return;
  }
  if (e.button === 2) return setTool(null);
  if (e.button !== 0 || !still) return;
  const p = renderer.pick(e.clientX, e.clientY);
  if (p) roads.click(p, e.shiftKey);
});
// The last cursor position over the canvas, so previews can follow a moving camera.
let pointer: { clientX: number; clientY: number } | null = null;
function hover(e: { clientX: number; clientY: number }) {
  pointer = e;
  if (zone !== null) {
    if (zoneStart) renderer.setZoneDrag(zoneStart, renderer.pick(e.clientX, e.clientY), zone);
    else {
      blockHover = renderer.pick(e.clientX, e.clientY);
      blockWanted = true;
      sendBlockPreview();
    }
    return;
  }
  if (roads.mode) roads.hover(renderer.pick(e.clientX, e.clientY), e);
}
canvas.addEventListener('pointermove', hover);
canvas.addEventListener('pointerleave', () => {
  pointer = null;
  if (!zoneStart) hideBlockOutline();
  roads.leave();
});
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

// --- keyboard camera: held W/A/S/D pan along the ground, Q/E turn, relative to the view ---
/** [right, forward, turn left] per key; speeds and easing live in `KeyCamera`. */
const CAMERA_KEYS: Record<string, [number, number, number]> = {
  w: [0, 1, 0],
  a: [-1, 0, 0],
  s: [0, -1, 0],
  d: [1, 0, 0],
  q: [0, 0, 1],
  e: [0, 0, -1],
};
const TEXT_ENTRY = 'input, textarea, select, [contenteditable]:not([contenteditable="false"])';
const heldCameraKeys = new Set<string>();
function moveWithKeys(seconds: number) {
  const input = [0, 0, 0];
  for (const key of heldCameraKeys) for (let i = 0; i < 3; i++) input[i] += CAMERA_KEYS[key][i];
  // Panning takes the camera back from follow; turning orbits the followed car.
  if (input[0] !== 0 || input[1] !== 0) inspector.setFollow(false);
  if (renderer.keyCamera.update(input[0], input[1], input[2], seconds) && zoneStart)
    zoneKeyPanned = true;
}
function stopKeyCamera() {
  heldCameraKeys.clear();
  renderer.keyCamera.stop();
}
// Keyup can go missing when the page loses focus or a modifier is down, so stop on those.
addEventListener('keyup', (e) => {
  if (e.key === 'Shift') roads.setShift(false);
  heldCameraKeys.delete(e.key.toLowerCase());
  ignoredCameraKeys.delete(e.key.toLowerCase());
});
addEventListener('blur', () => {
  stopKeyCamera();
  roads.setShift(false);
});
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') stopKeyCamera();
});
addEventListener('focusin', (e) => {
  if ((e.target as HTMLElement | null)?.closest?.(TEXT_ENTRY)) stopKeyCamera();
});

addEventListener('keydown', (e) => {
  // Holding Shift places road points freely (no angle, guide or length snapping).
  if (e.key === 'Shift') roads.setShift(true);
  // Keep browser shortcuts, held keys and text entry from toggling tools or panning.
  if (e.ctrlKey || e.metaKey || e.altKey) {
    stopKeyCamera();
    return;
  }
  if (e.isComposing) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest(TEXT_ENTRY)) return;
  const cameraKey = e.key.toLowerCase();
  if (Object.hasOwn(CAMERA_KEYS, cameraKey)) {
    if (!ignoredCameraKeys.has(cameraKey)) heldCameraKeys.add(cameraKey);
    return;
  }
  if (e.repeat) return;
  // Space activates a focused button natively; it must not also pause the sim.
  if (e.key === ' ' && target?.closest('button, a[href], [role="button"]')) return;
  if (views.key(e.key)) return;
  if (e.key === 'r' || e.key === 'R') toggleTool('straight');
  else if (e.key === 'v' || e.key === 'V') toggleTool('curve');
  else if (e.key === 'b' || e.key === 'B') toggleTool('bulldoze');
  else if (e.key === 'm' || e.key === 'M') toggleMainRoad();
  else if (e.key === 'z' || e.key === 'Z') zoneKey(Zone.ResidentialLow, Zone.ResidentialHigh);
  else if (e.key === 'x' || e.key === 'X') zoneKey(Zone.CommercialLow, Zone.CommercialHigh);
  else if (e.key === 'c' || e.key === 'C') zoneKey(Zone.IndustrialLow, Zone.IndustrialHigh);
  else if (e.key === 'u' || e.key === 'U') setZone(Zone.None);
  else if (e.key === 'f' || e.key === 'F') inspector.toggleFollow();
  else if (e.key === 'n' || e.key === 'N') sound.toggle();
  // Esc cancels a tool first, then closes the inspector.
  else if (e.key === 'Escape') roads.mode || zone !== null ? setTool(null) : inspector.select(null);
  else if (e.key === ' ') setSpeed(speed === 0 ? 1 : 0);
  else if (e.key === '1') setSpeed(1);
  else if (e.key === '2') setSpeed(3);
  else if (e.key === '3') setSpeed(10);
});

// --- frame loop ---
let fps = 60;
let lastFrame = performance.now();
function frame() {
  const now = performance.now();
  fps = fps * 0.95 + (1000 / Math.max(now - lastFrame, 1)) * 0.05;
  const seconds = (now - lastFrame) / 1000;
  moveWithKeys(seconds);
  lastFrame = now;
  if (curr) {
    const t = speed === 0 ? 1 : Math.min((now - currAt) / frameGap, 1);
    renderer.drawAgents(prev, curr, t, inspector.selected);
  }
  inspector.update(seconds);
  // A camera move slides the ground under a still cursor; ask again what the cursor is over.
  if (renderer.render() && pointer) hover(pointer);
  sound.update(now / 1000, curr, stats?.secondOfDay ?? 43200, renderer.listener());
  if (stats) {
    drawDemand(demand, demandFills, stats.demand);
    hud.textContent = hudText(stats, fps, tickMs, speed, zone);
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
if (import.meta.env.DEV) Object.assign(window, { renderer, sound });
