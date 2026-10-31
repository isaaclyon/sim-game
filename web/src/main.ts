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
let roadStart: THREE.Vector3 | null = null;
function setRoadTool(on: boolean) {
  roadTool = on;
  roadStart = null;
  renderer.setPreview(null, null);
  roadButton.classList.toggle('active', on);
}
roadButton.onclick = () => setRoadTool(!roadTool);

const canvas = renderer.renderer.domElement;
let downX = 0;
let downY = 0;
canvas.addEventListener('pointerdown', (e) => {
  downX = e.clientX;
  downY = e.clientY;
});
canvas.addEventListener('pointerup', (e) => {
  if (!roadTool) return;
  if (e.button === 2) return setRoadTool(false);
  // Treat it as a click only if the pointer barely moved, so panning still works.
  if (e.button !== 0 || Math.hypot(e.clientX - downX, e.clientY - downY) > 4) return;
  const p = renderer.pick(e.clientX, e.clientY);
  if (!p) return;
  if (roadStart) send({ type: 'buildRoad', ax: roadStart.x, ay: roadStart.z, bx: p.x, by: p.z });
  roadStart = p;
});
canvas.addEventListener('pointermove', (e) => {
  if (roadTool && roadStart) renderer.setPreview(roadStart, renderer.pick(e.clientX, e.clientY));
});
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

addEventListener('keydown', (e) => {
  if (e.key === 'r' || e.key === 'R') setRoadTool(!roadTool);
  else if (e.key === 'Escape') setRoadTool(false);
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
      (roadTool ? `\nROAD TOOL: click to place, Esc to stop` : '');
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

if (import.meta.env.DEV) Object.assign(window, { renderer });
