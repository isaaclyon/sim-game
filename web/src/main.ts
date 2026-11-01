import type * as THREE from 'three';
import type { FromWorker, Stats, ToWorker } from './protocol';
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
      if (msg.type === 'roadPreview' && roadTool) {
        renderer.setPreview(msg.stops, roadStart, previewRequestedEnd);
        refused = roadStart !== null && msg.stops.length === 0;
        updateRoadHelp();
      }
      if (msg.type === 'bulldozePreview' && bulldozeTool) renderer.setBulldozePreview(msg.segment);
    }
    sendPreview();
  } else if (msg.type === 'buildResult') {
    if (pendingBuild?.id !== msg.id) return;
    if (msg.success) {
      roadStart = pendingBuild.end;
      roadControl = null;
    }
    pendingBuild = null;
    refused = !msg.success;
    invalidatePreview();
    updateRoadHelp();
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

// Straight: start → end. Curve: start → bend control → end. Both chain after success.
const roadButton = byId('road');
const curveButton = byId('curve');
const roadHelp = byId('road-help');
let roadTool: 'straight' | 'curve' | null = null;
const bulldozeButton = byId('bulldoze');
let bulldozeTool = false;
let roadStart: THREE.Vector3 | null = null;
let roadControl: THREE.Vector3 | null = null;
let refused = false;
let nextBuildId = 0;
let pendingBuild: { id: number; end: THREE.Vector3 } | null = null;
let previewRevision = 0;
// The sim works out where a road snaps; the preview asks it: where the road would start while
// hovering, then the whole road once it has a start. Only one question is in flight at a time,
// and only the latest cursor position is asked about, so a busy worker never falls behind the
// mouse.
let previewEnd: THREE.Vector3 | null = null;
let previewWanted = false;
let previewInFlight = false;
let previewRequestedEnd: THREE.Vector3 | null = null;
function updateRoadHelp() {
  const stage = !roadStart
    ? 'Click the road start.'
    : roadTool === 'curve' && !roadControl
      ? 'Click the bend control point, then the road end.'
      : 'Click the road end.';
  roadHelp.textContent = bulldozeTool
    ? 'Click a highlighted road to remove it. Esc or right-click cancels.'
    : !roadTool
      ? ''
      : pendingBuild
        ? 'Building road…'
        : `${refused ? 'Cannot build here. Move the end away from overlaps or short joins. ' : ''}${stage} Esc or right-click cancels.`;
}
function invalidatePreview() {
  previewRevision++;
  previewEnd = null;
  previewWanted = false;
  renderer.setPreview(null, null, null);
}
function sendPreview() {
  if (!previewWanted || (!roadTool && !bulldozeTool) || !previewEnd || pendingBuild) return;
  const [a, b, c] = [roadStart, previewEnd, roadControl];
  if (a && roadTool === 'curve' && !c) {
    // A control guide, not a road. Only the sim evaluates the actual Bézier curve.
    previewWanted = false;
    renderer.setPreview(new Float32Array([a.x, a.z, 0, b.x, b.z, -1]), a, b);
    return;
  }
  if (previewInFlight) return;
  previewWanted = false;
  previewInFlight = true;
  previewRequestedEnd = b;
  const revision = previewRevision;
  send(
    bulldozeTool
      ? { type: 'previewBulldoze', revision, x: b.x, y: b.z }
      : a
        ? c
          ? { type: 'previewCurve', revision, ax: a.x, ay: a.z, cx: c.x, cy: c.z, bx: b.x, by: b.z }
          : { type: 'previewRoad', revision, ax: a.x, ay: a.z, bx: b.x, by: b.z }
        : { type: 'previewSnap', revision, x: b.x, y: b.z },
  );
}
function setRoadTool(tool: typeof roadTool | 'bulldoze') {
  roadTool = tool === 'bulldoze' ? null : tool;
  bulldozeTool = tool === 'bulldoze';
  roadStart = null;
  roadControl = null;
  pendingBuild = null;
  refused = false;
  invalidatePreview();
  for (const [button, mode] of [
    [roadButton, 'straight'],
    [curveButton, 'curve'],
    [bulldozeButton, 'bulldoze'],
  ] as const) {
    button.classList.toggle('active', tool === mode);
    button.setAttribute('aria-pressed', String(tool === mode));
  }
  renderer.renderer.domElement.style.cursor = tool ? 'crosshair' : '';
  updateRoadHelp();
}
roadButton.onclick = () => setRoadTool(roadTool === 'straight' ? null : 'straight');
curveButton.onclick = () => setRoadTool(roadTool === 'curve' ? null : 'curve');
bulldozeButton.onclick = () => setRoadTool(bulldozeTool ? null : 'bulldoze');

const canvas = renderer.renderer.domElement;
let downX = 0;
let downY = 0;
canvas.addEventListener('pointerdown', (e) => {
  downX = e.clientX;
  downY = e.clientY;
});
canvas.addEventListener('pointerup', (e) => {
  if (!roadTool && !bulldozeTool) return;
  if (e.button === 2) return setRoadTool(null);
  if (pendingBuild) return;
  // Treat it as a click only if the pointer barely moved, so panning still works.
  if (e.button !== 0 || Math.hypot(e.clientX - downX, e.clientY - downY) > 4) return;
  const p = renderer.pick(e.clientX, e.clientY);
  if (!p) return;
  if (bulldozeTool) send({ type: 'deleteRoad', x: p.x, y: p.z });
  else if (!roadStart) roadStart = p;
  else if (roadTool === 'curve' && !roadControl) roadControl = p;
  else {
    const id = ++nextBuildId;
    pendingBuild = { id, end: p };
    const a = roadStart;
    const c = roadControl;
    send(
      c
        ? { type: 'buildCurve', id, ax: a.x, ay: a.z, cx: c.x, cy: c.z, bx: p.x, by: p.z }
        : { type: 'buildRoad', id, ax: a.x, ay: a.z, bx: p.x, by: p.z },
    );
  }
  refused = false;
  invalidatePreview();
  updateRoadHelp();
});
canvas.addEventListener('pointermove', (e) => {
  if ((!roadTool && !bulldozeTool) || pendingBuild) return;
  // Keep the latest completed preview visible during movement. Only a change of tool or
  // construction stage invalidates an in-flight response; cursor updates queue the next one.
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
  if (e.key === 'r' || e.key === 'R') setRoadTool(roadTool === 'straight' ? null : 'straight');
  else if (e.key === 'c' || e.key === 'C') setRoadTool(roadTool === 'curve' ? null : 'curve');
  else if (e.key === 'b' || e.key === 'B') setRoadTool(bulldozeTool ? null : 'bulldoze');
  else if (e.key === 'Escape') setRoadTool(null);
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
      `road edits ${fmt(stats.despawnedByEdit)} cars removed\n`;
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

if (import.meta.env.DEV) Object.assign(window, { renderer });
