import type * as THREE from 'three';
import { type FromWorker, type Stats, type ToWorker, Zone } from './protocol';
import { Renderer } from './renderer';

const byId = (id: string): HTMLElement => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing #${id} in index.html`);
  return el;
};

const renderer = new Renderer(document.body);
const hud = byId('hud');
const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' });
const send = (msg: ToWorker) => worker.postMessage(msg);

// Two most recent snapshots, and when they arrived, for interpolation.
let prev: Float32Array | undefined;
let curr: Float32Array | undefined;
let currAt = 0;
let frameGap = 100;
let stats: Stats | undefined;
let tickMs = 0;
let roadsSeen = false;

worker.onmessage = (e: MessageEvent<FromWorker>) => {
  const msg = e.data;
  if (msg.type === 'roads') {
    renderer.setRoads(msg.segments, msg.bounds, !roadsSeen);
    roadsSeen = true;
    // Retry a hover sent before WASM was ready, or after the topology changed.
    if (previewEnd) {
      previewRevision++;
      previewWanted = true;
      sendPreview();
    }
  } else if (msg.type === 'roadPreview' || msg.type === 'bulldozePreview') {
    previewInFlight = false;
    if (msg.revision === previewRevision && previewEnd) {
      if (msg.type === 'roadPreview' && roadTool)
        renderer.setPreview(msg.stops, roadStart, previewEnd);
      if (msg.type === 'bulldozePreview' && bulldozeTool) renderer.setBulldozePreview(msg.segment);
    }
    sendPreview();
  } else if (msg.type === 'zones') {
    renderer.setZones(msg.cells, msg.layout);
  } else {
    const now = performance.now();
    if (curr) frameGap = frameGap * 0.8 + (now - currAt) * 0.2;
    prev = curr;
    curr = msg.agents;
    currAt = now;
    stats = msg.stats;
    tickMs = msg.tickMs;
    renderer.drawSignals(msg.signals);
  }
};

send({ type: 'init', seed: 1, cols: 60, rows: 60, agents: 50_000 });

// --- speed controls ---
let speed = 1;
const speedButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-speed]')];
function setSpeed(s: number) {
  speed = s;
  send({ type: 'setSpeed', speed: s });
  for (const b of speedButtons) b.classList.toggle('active', Number(b.dataset.speed) === s);
}
for (const b of speedButtons) b.onclick = () => setSpeed(Number(b.dataset.speed));
byId('spawn').onclick = () => send({ type: 'spawn', count: 5000 });

// --- road tool: click to start, click to place (chains), Esc or right-click to stop ---
const roadButton = byId('road');
let roadTool = false;
const bulldozeButton = byId('bulldoze');
let bulldozeTool = false;
// --- zone brush: pick a zone, then drag a rectangle; every cell inside takes that zone ---
const zoneButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-zone]')];
const ZONE_NAMES = [
  'unzone',
  'residential low',
  'residential high',
  'commercial low',
  'commercial high',
  'industrial low',
  'industrial high',
];
let zone: Zone | null = null;
let zoneStart: THREE.Vector3 | null = null;
let previewRevision = 0;
let roadStart: THREE.Vector3 | null = null;
// The sim works out where a road snaps; the preview asks it: where the road would start while
// hovering, then the whole road once it has a start. Only one question is in flight at a time,
// and only the latest cursor position is asked about, so a busy worker never falls behind the
// mouse.
let previewEnd: THREE.Vector3 | null = null;
let previewWanted = false;
let previewInFlight = false;
function sendPreview() {
  if (previewInFlight || !previewWanted || (!roadTool && !bulldozeTool) || !previewEnd) return;
  previewWanted = false;
  previewInFlight = true;
  const [a, b] = [roadStart, previewEnd];
  const revision = previewRevision;
  send(
    bulldozeTool
      ? { type: 'previewBulldoze', x: b.x, y: b.z, revision }
      : a
        ? { type: 'previewRoad', ax: a.x, ay: a.z, bx: b.x, by: b.z, revision }
        : { type: 'previewSnap', x: b.x, y: b.z, revision },
  );
}
function setTool(tool: 'road' | 'bulldoze' | null, brush: Zone | null = null) {
  roadTool = tool === 'road';
  bulldozeTool = tool === 'bulldoze';
  zone = tool ? null : brush;
  zoneStart = null;
  renderer.setZoneDrag(null, null, 0);
  renderer.setLeftDragPans(zone === null);
  for (const b of zoneButtons) {
    const on = Number(b.dataset.zone) === zone;
    b.classList.toggle('active', on);
    b.setAttribute('aria-pressed', String(on));
  }
  previewRevision++;
  previewWanted = false;
  roadStart = null;
  previewEnd = null;
  renderer.setPreview(null, null, null);
  roadButton.classList.toggle('active', roadTool);
  bulldozeButton.classList.toggle('active', bulldozeTool);
  roadButton.setAttribute('aria-pressed', String(roadTool));
  bulldozeButton.setAttribute('aria-pressed', String(bulldozeTool));
  renderer.renderer.domElement.style.cursor = tool || zone !== null ? 'crosshair' : '';
}
function setZone(z: Zone) {
  setTool(null, zone === z ? null : z);
}
for (const b of zoneButtons) b.onclick = () => setZone(Number(b.dataset.zone) as Zone);
/** Z, X and C pick residential, commercial and industrial; pressing again switches density. */
function zoneKey(low: Zone, high: Zone) {
  setTool(null, zone === low ? high : low);
}
roadButton.onclick = () => setTool(roadTool ? null : 'road');
bulldozeButton.onclick = () => setTool(bulldozeTool ? null : 'bulldoze');

const canvas = renderer.renderer.domElement;
let downX = 0;
let downY = 0;
canvas.addEventListener('pointerdown', (e) => {
  downX = e.clientX;
  downY = e.clientY;
  if (zone === null || e.button !== 0) return;
  zoneStart = renderer.pick(e.clientX, e.clientY);
  // Keep getting moves and the release even if the drag leaves the canvas.
  canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener('pointerup', (e) => {
  if (zone !== null) {
    if (e.button === 2) return setTool(null);
    const end = renderer.pick(e.clientX, e.clientY);
    if (e.button === 0 && zoneStart && end) {
      const [a, b] = [zoneStart, end];
      send({ type: 'paintZone', ax: a.x, ay: a.z, bx: b.x, by: b.z, zone });
    }
    zoneStart = null;
    renderer.setZoneDrag(null, null, 0);
    return;
  }
  if (!roadTool && !bulldozeTool) return;
  if (e.button === 2) return setTool(null);
  // Treat it as a click only if the pointer barely moved, so panning still works.
  if (e.button !== 0 || Math.hypot(e.clientX - downX, e.clientY - downY) > 4) return;
  const p = renderer.pick(e.clientX, e.clientY);
  if (!p) return;
  previewRevision++;
  if (bulldozeTool) {
    send({ type: 'deleteRoad', x: p.x, y: p.z });
  } else {
    if (roadStart) send({ type: 'buildRoad', ax: roadStart.x, ay: roadStart.z, bx: p.x, by: p.z });
    roadStart = p;
  }
  previewEnd = null;
  renderer.setPreview(null, null, null);
});
canvas.addEventListener('pointermove', (e) => {
  if (zone !== null && zoneStart) {
    renderer.setZoneDrag(zoneStart, renderer.pick(e.clientX, e.clientY), zone);
    return;
  }
  if (!roadTool && !bulldozeTool) return;
  // Keep accepting replies while moving; otherwise a busy worker never draws a preview.
  previewEnd = renderer.pick(e.clientX, e.clientY);
  previewWanted = true;
  sendPreview();
});
canvas.addEventListener('pointerleave', () => {
  previewRevision++;
  previewEnd = null;
  previewWanted = false;
  renderer.setPreview(null, null, null);
});
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

addEventListener('keydown', (e) => {
  if (e.key === 'r' || e.key === 'R') setTool(roadTool ? null : 'road');
  else if (e.key === 'b' || e.key === 'B') setTool(bulldozeTool ? null : 'bulldoze');
  else if (e.key === 'z' || e.key === 'Z') zoneKey(Zone.ResidentialLow, Zone.ResidentialHigh);
  else if (e.key === 'x' || e.key === 'X') zoneKey(Zone.CommercialLow, Zone.CommercialHigh);
  else if (e.key === 'c' || e.key === 'C') zoneKey(Zone.IndustrialLow, Zone.IndustrialHigh);
  else if (e.key === 'u' || e.key === 'U') setZone(Zone.None);
  else if (e.key === 'Escape') setTool(null);
  else if (e.key === ' ') setSpeed(speed === 0 ? 1 : 0);
  else if (e.key === '1') setSpeed(1);
  else if (e.key === '2') setSpeed(3);
  else if (e.key === '3') setSpeed(10);
});

// --- frame loop ---
function fmt(n: number) {
  return n.toLocaleString('en-US');
}
let fps = 60;
let lastFrame = performance.now();
function frame() {
  const now = performance.now();
  fps = fps * 0.95 + (1000 / Math.max(now - lastFrame, 1)) * 0.05;
  lastFrame = now;
  if (curr) {
    const t = speed === 0 ? 1 : Math.min((now - currAt) / frameGap, 1);
    renderer.drawAgents(prev, curr, t);
  }
  renderer.render();
  if (stats) {
    const simSecs = stats.tick / 10;
    const clock = `${Math.floor(simSecs / 60)}m ${Math.floor(simSecs % 60)}s`;
    hud.textContent =
      `sim time   ${clock}   speed ${speed}×\n` +
      `fps        ${fps.toFixed(0)}   tick ${tickMs.toFixed(2)} ms\n` +
      `agents     ${fmt(stats.agents)}\n` +
      `driving    ${fmt(stats.driving)}   parked ${fmt(stats.parked)}\n` +
      `trips      ${fmt(stats.tripsCompleted)}\n` +
      `stuck      ${fmt(stats.despawnedStuck)}   no path ${fmt(stats.pathsFailed)}\n` +
      `road edits ${fmt(stats.despawnedByEdit)} cars removed\n` +
      (roadTool ? `\nROAD TOOL: click to place, Esc to stop` : '') +
      (bulldozeTool ? `\nBULLDOZE: click a highlighted road to remove it, Esc to stop` : '') +
      (zone !== null
        ? `\nZONE: ${ZONE_NAMES[zone]}. Drag a rectangle to paint, Esc to stop` +
          `\n(Z/X/C pick a zone, press again for the other density; U unzones)`
        : '');
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

if (import.meta.env.DEV) Object.assign(window, { renderer });
