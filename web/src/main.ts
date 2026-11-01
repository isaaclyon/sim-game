import type * as THREE from 'three';
import { type FromWorker, RoadClass, type Stats, type ToWorker, Zone } from './protocol';
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
// Keyboard panning during a zone press moves the ground under the cursor: that is a drag.
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
  if (!roadTool && !bulldozeTool) return;
  if (e.button === 2) return setTool(null);
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

// --- keyboard camera: held W/A/S/D pan along the ground, relative to the view ---
/** Screen-relative [right, forward] per key. */
const PAN_KEYS: Record<string, [number, number]> = { w: [0, 1], a: [-1, 0], s: [0, -1], d: [1, 0] };
/** Camera distances per second, so the ground seems to move at the same pace at any zoom. */
const KEY_PAN_SPEED = 1;
/** Longest frame step that pans, so a stalled or background tab cannot jump the camera. */
const MAX_PAN_STEP = 0.1;
const TEXT_ENTRY = 'input, textarea, select, [contenteditable]:not([contenteditable="false"])';
const heldPanKeys = new Set<string>();
function panWithKeys(seconds: number) {
  let right = 0;
  let forward = 0;
  for (const key of heldPanKeys) {
    right += PAN_KEYS[key][0];
    forward += PAN_KEYS[key][1];
  }
  const length = Math.hypot(right, forward);
  if (length === 0) return;
  const step = (KEY_PAN_SPEED * Math.min(seconds, MAX_PAN_STEP)) / length;
  renderer.panGround(right * step, forward * step);
  if (zoneStart) zoneKeyPanned = true;
}
// Keyup can go missing when the page loses focus or a modifier is down, so release on those.
addEventListener('keyup', (e) => heldPanKeys.delete(e.key.toLowerCase()));
addEventListener('blur', () => heldPanKeys.clear());
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') heldPanKeys.clear();
});
addEventListener('focusin', (e) => {
  if ((e.target as HTMLElement | null)?.closest?.(TEXT_ENTRY)) heldPanKeys.clear();
});

addEventListener('keydown', (e) => {
  // Keep browser shortcuts, held keys and text entry from toggling tools or panning.
  if (e.ctrlKey || e.metaKey || e.altKey) {
    heldPanKeys.clear();
    return;
  }
  if (e.isComposing) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest(TEXT_ENTRY)) return;
  const panKey = e.key.toLowerCase();
  if (Object.hasOwn(PAN_KEYS, panKey)) {
    heldPanKeys.add(panKey);
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
  panWithKeys((now - lastFrame) / 1000);
  lastFrame = now;
  if (curr) {
    const t = speed === 0 ? 1 : Math.min((now - currAt) / frameGap, 1);
    renderer.drawAgents(prev, curr, t);
  }
  // A camera move slides the ground under a still cursor; ask again what the cursor is over.
  if (renderer.render() && pointer) hover(pointer);
  if (stats) {
    const hh = String(Math.floor(stats.secondOfDay / 3600)).padStart(2, '0');
    const mm = String(Math.floor((stats.secondOfDay % 3600) / 60)).padStart(2, '0');
    hud.textContent =
      `Day ${stats.day}   ${hh}:${mm}   ${speed === 0 ? 'paused' : `${speed}×`}\n` +
      `fps        ${fps.toFixed(0)}   tick ${tickMs.toFixed(2)} ms\n` +
      `population ${fmt(stats.population)}\n` +
      `driving    ${fmt(stats.driving)}   parked ${fmt(stats.parked)}\n` +
      `trips      ${fmt(stats.tripsCompleted)}\n` +
      `stuck      ${fmt(stats.despawnedStuck)}   no path ${fmt(stats.pathsFailed)}\n` +
      `road edits ${fmt(stats.despawnedByEdit)} cars removed\n` +
      `camera     WASD or drag pans, right-drag orbits, wheel zooms\n` +
      (zone !== null
        ? `\nZONE: ${ZONE_NAMES[zone]}. Click a block to fill it, drag a rectangle to paint, Esc to stop` +
          `\n(Z/X/C pick a zone, press again for the other density; U unzones)`
        : '');
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

if (import.meta.env.DEV) Object.assign(window, { renderer });
