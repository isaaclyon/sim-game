import { AmbientSound } from './ambient-sound';
import { CityControls } from './city-controls';
import { Inspector } from './inspector';
import type { FromWorker, Stats, ToWorker } from './protocol';
import { Renderer } from './renderer';
import { Toolbox } from './toolbox';
import { BuildMenu } from './ui/build-menu';
import { BULLDOZE, CATEGORIES } from './ui/catalog';
import { DebugPanel } from './ui/debug-panel';
import { Popovers } from './ui/popover';
import { TopBar } from './ui/top-bar';
import { Views } from './ui/views';

const byId = (id: string): HTMLElement => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing #${id} in index.html`);
  return el;
};

const renderer = new Renderer(document.body);
const sound = new AmbientSound(byId('sound') as HTMLButtonElement);
const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' });
const send = (msg: ToWorker) => worker.postMessage(msg);
const cityControls = new CityControls(send, byId, import.meta.env.DEV);
const popovers = new Popovers();
const debug = new DebugPanel(byId('debug'));
// The build menu starts tools through the toolbox, which tells the menu what is in use.
const menu = new BuildMenu(
  byId('build-menu'),
  CATEGORIES,
  BULLDOZE,
  {
    pick: (item) => tools.use(item?.tool ?? null),
    option: (option) => tools.option(option.action),
  },
  popovers,
);
const views = new Views(menu.viewsButton, renderer, popovers);
const tools = new Toolbox(
  send,
  renderer,
  { hint: menu.hint, readout: byId('road-readout') },
  views,
  () => menu.show(tools.selection),
);
const { roads, zoning, power } = tools;
const topBar = new TopBar(byId, popovers, setSpeed, (id, focus) => menu.openCategory(id, focus));
// Each item's picture is drawn once from the game's own models (M4.6); the debug panel shows
// how long that added to startup.
const picturesStart = performance.now();
menu.setPictures(renderer.drawPictures(menu.pictures));
debug.picturesMs = performance.now() - picturesStart;
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
  {
    panel: byId('inspector'),
    title: byId('inspector-title'),
    lines: byId('inspector-lines'),
    follow: byId('follow') as HTMLButtonElement,
    close: byId('inspector-close') as HTMLButtonElement,
  },
);
renderer.onUserPan = () => inspector.setFollow(false);

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
  if (power.receive(msg)) return;
  if (msg.type === 'cityStatus') {
    cityControls.receive(msg);
    topBar.setCity(msg);
  } else if (msg.type === 'cityFile' || msg.type === 'commandLog') {
    void cityControls.receiveFile(msg);
  } else if (msg.type === 'cityReset') {
    tools.use(null);
    menu.closeCategory();
    inspector.select(null);
    stopKeyCamera();
    pointer = null;
    zoning.reset();
    roads.reset();
    prev = curr = undefined;
    stats = undefined;
    roadsSeen = zonesSeen = buildingsSeen = false;
    frameGap = 100;
    setSpeed(msg.speed);
  } else if (msg.type === 'roads') {
    renderer.setRoads(msg.segments, msg.bounds, !roadsSeen);
    roadsSeen = true;
    zoning.roadsChanged();
    // Retry a hover sent before WASM was ready, or after the topology changed.
    roads.roadsChanged();
  } else if (
    msg.type === 'roadPreview' ||
    msg.type === 'gridPreview' ||
    msg.type === 'bulldozePreview' ||
    msg.type === 'roadClassPreview' ||
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
    zoning.receive(msg);
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
function setSpeed(s: number) {
  speed = s;
  send({ type: 'setSpeed', speed: s });
  topBar.showSpeed(s);
}

const canvas = renderer.renderer.domElement;
let downX = 0;
let downY = 0;
canvas.addEventListener('pointerdown', (e) => {
  downX = e.clientX;
  downY = e.clientY;
  if (zoning.zone === null || e.button !== 0) return;
  zoning.press(renderer.pick(e.clientX, e.clientY));
  // Keep getting moves and the release even if the drag leaves the canvas.
  canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener('pointerup', (e) => {
  // Treat it as a click only if the pointer barely moved, so panning still works.
  const still = Math.hypot(e.clientX - downX, e.clientY - downY) <= 4;
  if (zoning.zone !== null) {
    if (e.button === 2) return tools.use(null);
    return zoning.release(e.button, still, renderer.pick(e.clientX, e.clientY));
  }
  if (power.active) return power.release(e.button, still, renderer.pick(e.clientX, e.clientY));
  if (!roads.mode) {
    if (e.button === 0 && still) {
      const agent = renderer.carAt(e.clientX, e.clientY);
      const building = agent === null ? renderer.buildingAt(e.clientX, e.clientY) : null;
      if (building !== null) inspector.selectBuilding(building);
      else inspector.select(agent);
    }
    return;
  }
  if (e.button === 2) return tools.use(null);
  if (e.button !== 0 || !still) return;
  const p = renderer.pick(e.clientX, e.clientY);
  if (p) roads.click(p, e.shiftKey);
});
// The last cursor position over the canvas, so previews can follow a moving camera.
let pointer: { clientX: number; clientY: number } | null = null;
function hover(e: { clientX: number; clientY: number }) {
  pointer = e;
  if (zoning.zone !== null) return zoning.hover(renderer.pick(e.clientX, e.clientY));
  if (power.active) return power.hover(renderer.pick(e.clientX, e.clientY));
  if (roads.mode) roads.hover(renderer.pick(e.clientX, e.clientY), e);
}
canvas.addEventListener('pointermove', hover);
canvas.addEventListener('pointerleave', () => {
  pointer = null;
  power.hover(null);
  zoning.leave();
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
  if (renderer.keyCamera.update(input[0], input[1], input[2], seconds)) zoning.cameraMoved();
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

/** Esc closes an open panel first, then puts the tool away, closes the item row, and last
 * closes the inspector. */
function cancel() {
  if (popovers.close() || tools.cancel() || menu.closeCategory()) return;
  inspector.select(null);
}

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
  if (target?.closest(TEXT_ENTRY)) {
    // Esc still closes the panel holding the field, such as the ☰ menu's share link.
    if (e.key === 'Escape') popovers.close();
    return;
  }
  const cameraKey = e.key.toLowerCase();
  if (Object.hasOwn(CAMERA_KEYS, cameraKey)) {
    if (!ignoredCameraKeys.has(cameraKey)) heldCameraKeys.add(cameraKey);
    return;
  }
  if (e.repeat) return;
  // Space activates a focused button natively; it must not also pause the sim.
  if (e.key === ' ' && target?.closest('button, a[href], [role="button"]')) return;
  if (e.key === 'Escape') return cancel();
  if (debug.key(e.key) || tools.key(e.key) || views.key(e.key)) return;
  const key = e.key.toLowerCase();
  if (key === 'f') inspector.toggleFollow();
  else if (key === 'n') sound.toggle();
  else if (key === ' ') setSpeed(speed === 0 ? 1 : 0);
  else if (key === '1') setSpeed(1);
  else if (key === '2') setSpeed(3);
  else if (key === '3') setSpeed(10);
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
  topBar.draw(stats, Date.now());
  debug.draw(stats, fps, tickMs);
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
if (import.meta.env.DEV) Object.assign(window, { renderer, sound });
