import type * as THREE from 'three';
import { AmbientSound } from './ambient-sound';
import { drawDemand } from './demand-meter';
import { Inspector } from './inspector';
import { type FromWorker, RoadClass, type Stats, type ToWorker, Zone } from './protocol';
import { Renderer } from './renderer';

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
const trafficButton = byId('traffic');
const toggleTraffic = () =>
  trafficButton.setAttribute('aria-pressed', String(renderer.toggleTrafficView()));
trafficButton.onclick = toggleTraffic;

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
    // Blocks change with roads, including alleys the sim adds after zoning.
    if (blockHover) {
      blockWanted = true;
      sendBlockPreview();
    }
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
  } else if (msg.type === 'zones') {
    renderer.setZones(msg.cells, msg.layout);
  } else if (msg.type === 'lots') {
    renderer.setLots(msg.rings);
  } else if (msg.type === 'buildings') {
    renderer.setBuildings(msg.data);
    sound.setBuildings(msg.data);
  } else if (msg.type === 'citizen') {
    inspector.receive(msg.selection, msg.citizen);
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
    renderer.drawSignals(msg.signals);
  }
};

send({ type: 'init', seed: 1, cols: 60, rows: 60, agents: 0 });

// --- speed controls ---
let speed = 1;
const speedButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-speed]')];
function setSpeed(s: number) {
  speed = s;
  send({ type: 'setSpeed', speed: s });
  for (const b of speedButtons) b.classList.toggle('active', Number(b.dataset.speed) === s);
}
for (const b of speedButtons) b.onclick = () => setSpeed(Number(b.dataset.speed));

// Straight: start → end. Curve: start → bend control → end. Both chain after success.
const roadButton = byId('road');
const curveButton = byId('curve');
const roadHelp = byId('road-help');
let roadTool: 'straight' | 'curve' | null = null;
// Main road (M) is a setting of both road tools, kept until switched off.
const mainRoadButton = byId('main-road');
let roadClass: RoadClass = RoadClass.Street;
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
        : `${roadClass === RoadClass.Main ? 'Main road. ' : ''}${refused ? 'Cannot build here. Move the end away from overlaps or short joins. ' : ''}${stage} Esc or right-click cancels.`;
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
function setTool(tool: typeof roadTool | 'bulldoze', brush: Zone | null = null) {
  roadTool = tool === 'bulldoze' ? null : tool;
  bulldozeTool = tool === 'bulldoze';
  zone = tool ? null : brush;
  zoneStart = null;
  hideBlockOutline();
  renderer.setLeftDragPans(zone === null);
  for (const b of zoneButtons) {
    const on = Number(b.dataset.zone) === zone;
    b.classList.toggle('active', on);
    b.setAttribute('aria-pressed', String(on));
  }
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
  renderer.renderer.domElement.style.cursor = tool || zone !== null ? 'crosshair' : '';
  updateRoadHelp();
}
/** Toggles main road for the road tools, picking the straight road tool if none is active. */
function toggleMainRoad() {
  roadClass = roadClass === RoadClass.Main ? RoadClass.Street : RoadClass.Main;
  const on = roadClass === RoadClass.Main;
  mainRoadButton.classList.toggle('active', on);
  mainRoadButton.setAttribute('aria-pressed', String(on));
  if (!roadTool) setTool('straight');
  else updateRoadHelp();
}
function setZone(z: Zone) {
  setTool(null, zone === z ? null : z);
}
for (const b of zoneButtons) b.onclick = () => setZone(Number(b.dataset.zone) as Zone);
/** Z, X and C pick residential, commercial and industrial; pressing again switches density. */
function zoneKey(low: Zone, high: Zone) {
  setTool(null, zone === low ? high : low);
}
roadButton.onclick = () => setTool(roadTool === 'straight' ? null : 'straight');
curveButton.onclick = () => setTool(roadTool === 'curve' ? null : 'curve');
bulldozeButton.onclick = () => setTool(bulldozeTool ? null : 'bulldoze');
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
  if (!roadTool && !bulldozeTool) {
    if (e.button === 0 && still) inspector.select(renderer.carAt(e.clientX, e.clientY));
    return;
  }
  if (e.button === 2) return setTool(null);
  if (pendingBuild) return;
  if (e.button !== 0 || !still) return;
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
    const cls = roadClass;
    send(
      c
        ? {
            type: 'buildCurve',
            id,
            ax: a.x,
            ay: a.z,
            cx: c.x,
            cy: c.z,
            bx: p.x,
            by: p.z,
            class: cls,
          }
        : { type: 'buildRoad', id, ax: a.x, ay: a.z, bx: p.x, by: p.z, class: cls },
    );
  }
  refused = false;
  invalidatePreview();
  updateRoadHelp();
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
  if ((!roadTool && !bulldozeTool) || pendingBuild) return;
  // Keep the latest completed preview visible during movement. Only a change of tool or
  // construction stage invalidates an in-flight response; cursor updates queue the next one.
  previewEnd = renderer.pick(e.clientX, e.clientY);
  previewWanted = true;
  sendPreview();
}
canvas.addEventListener('pointermove', hover);
canvas.addEventListener('pointerleave', () => {
  pointer = null;
  if (!zoneStart) hideBlockOutline();
  previewRevision++;
  previewEnd = null;
  previewWanted = false;
  renderer.setPreview(null, null, null);
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
  heldCameraKeys.delete(e.key.toLowerCase());
  ignoredCameraKeys.delete(e.key.toLowerCase());
});
addEventListener('blur', stopKeyCamera);
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') stopKeyCamera();
});
addEventListener('focusin', (e) => {
  if ((e.target as HTMLElement | null)?.closest?.(TEXT_ENTRY)) stopKeyCamera();
});

addEventListener('keydown', (e) => {
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
  if (e.key === 'r' || e.key === 'R') setTool(roadTool === 'straight' ? null : 'straight');
  else if (e.key === 'v' || e.key === 'V') setTool(roadTool === 'curve' ? null : 'curve');
  else if (e.key === 'b' || e.key === 'B') setTool(bulldozeTool ? null : 'bulldoze');
  else if (e.key === 'm' || e.key === 'M') toggleMainRoad();
  else if (e.key === 'z' || e.key === 'Z') zoneKey(Zone.ResidentialLow, Zone.ResidentialHigh);
  else if (e.key === 'x' || e.key === 'X') zoneKey(Zone.CommercialLow, Zone.CommercialHigh);
  else if (e.key === 'c' || e.key === 'C') zoneKey(Zone.IndustrialLow, Zone.IndustrialHigh);
  else if (e.key === 'u' || e.key === 'U') setZone(Zone.None);
  else if (e.key === 'f' || e.key === 'F') inspector.toggleFollow();
  else if (e.key === 'n' || e.key === 'N') sound.toggle();
  else if (e.key === 't' || e.key === 'T') toggleTraffic();
  // Esc cancels a tool first, then closes the inspector.
  else if (e.key === 'Escape')
    roadTool || bulldozeTool || zone !== null ? setTool(null) : inspector.select(null);
  else if (e.key === ' ') setSpeed(speed === 0 ? 1 : 0);
  else if (e.key === '1') setSpeed(1);
  else if (e.key === '2') setSpeed(3);
  else if (e.key === '3') setSpeed(10);
});

// --- frame loop ---
const fmt = (n: number) => n.toLocaleString('en-US');
/** Share of finished trips made on foot (ADR 0026). */
const walkShare = (s: Stats) =>
  `${s.tripsCompleted ? ((100 * s.walksCompleted) / s.tripsCompleted).toFixed(0) : 0}%`;
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
    const hh = String(Math.floor(stats.secondOfDay / 3600)).padStart(2, '0');
    const mm = String(Math.floor((stats.secondOfDay % 3600) / 60)).padStart(2, '0');
    hud.textContent =
      `Day ${stats.day}   ${hh}:${mm}   ${speed === 0 ? 'paused' : `${speed}×`}\n` +
      `fps        ${fps.toFixed(0)}   tick ${tickMs.toFixed(2)} ms\n` +
      `population ${fmt(stats.population)}   unemployed ${fmt(stats.unemployed)} (${stats.unemploymentPercent.toFixed(1)}%)\n` +
      `driving    ${fmt(stats.driving)}   walking ${fmt(stats.walking)}   parked ${fmt(stats.parked)}\n` +
      `trips      ${fmt(stats.tripsCompleted)}   on foot ${walkShare(stats)}\n` +
      `stuck      ${fmt(stats.despawnedStuck)}   no path ${fmt(stats.pathsFailed)}\n` +
      `road edits ${fmt(stats.despawnedByEdit)} cars removed\n` +
      `camera     WASD or drag pans, Q/E turns, right-drag orbits, wheel zooms\n` +
      `inspect    click a car or walker; F follows, Esc closes   N sound   T traffic\n` +
      (zone !== null
        ? `\nZONE: ${ZONE_NAMES[zone]}. Click a block to fill it, drag a rectangle to paint, Esc to stop` +
          `\n(Z/X/C pick a zone, press again for the other density; U unzones)`
        : '');
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

if (import.meta.env.DEV) Object.assign(window, { renderer, sound });
