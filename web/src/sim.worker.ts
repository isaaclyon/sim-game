// Runs the WASM sim on a fixed-timestep clock, off the main thread.

import init, { Sim } from './pkg/sim_wasm';
import type { FromWorker, Stats, ToWorker } from './protocol';

/** If the sim falls this far behind, drop the backlog instead of spiralling. */
const MAX_TICKS_PER_STEP = 20;

let sim: Sim | undefined;
let tickMs = 100;
let speed = 1;
let backlogMs = 0;
let last = performance.now();
let roadsVersion = -1;
let zonesVersion = -1;
let lotsVersion = -1;

function post(msg: FromWorker, transfer: Transferable[] = []) {
  self.postMessage(msg, { transfer });
}

function sendRoadsIfChanged(s: Sim) {
  const version = s.roads_version();
  if (version === roadsVersion) return;
  roadsVersion = version;
  const segments = s.road_segments();
  const [x0, y0, x1, y1] = s.bounds();
  post({ type: 'roads', segments, bounds: [x0, y0, x1, y1] }, [segments.buffer]);
}

function sendZonesIfChanged(s: Sim) {
  const version = s.zones_version();
  if (version === zonesVersion) return;
  zonesVersion = version;
  const cells = s.zone_cells();
  const [x0, y0, cell, cols, rows] = s.zone_layout();
  post({ type: 'zones', cells, layout: [x0, y0, cell, cols, rows] }, [cells.buffer]);
}

function sendLotsIfChanged(s: Sim) {
  const version = s.lots_version();
  if (version === lotsVersion) return;
  lotsVersion = version;
  const segments = s.lot_outlines();
  post({ type: 'lots', segments }, [segments.buffer]);
}

/** After an edit: zoning can add alleys, and road edits can recut lots (ADR 0016). */
function sendEdits(s: Sim) {
  sendRoadsIfChanged(s);
  sendZonesIfChanged(s);
  sendLotsIfChanged(s);
  sendFrame(s); // Cars and signals also change while paused.
}

function readStats(s: Sim): Stats {
  const st = s.stats();
  const out: Stats = {
    tick: st.tick,
    agents: st.agents,
    parked: st.parked,
    awaitingPath: st.awaiting_path,
    entering: st.entering,
    driving: st.driving,
    tripsCompleted: st.trips_completed,
    pathsFailed: st.paths_failed,
    despawnedStuck: st.despawned_stuck,
    despawnedByEdit: st.despawned_by_edit,
  };
  st.free();
  return out;
}

function sendFrame(s: Sim, perTick = 0) {
  const agents = s.agent_snapshot();
  const signals = s.signal_snapshot();
  post({ type: 'frame', agents, signals, stats: readStats(s), tickMs: perTick }, [
    agents.buffer,
    signals.buffer,
  ]);
}

function step() {
  const now = performance.now();
  backlogMs += (now - last) * speed;
  last = now;

  if (sim) {
    let n = Math.floor(backlogMs / tickMs);
    if (n > MAX_TICKS_PER_STEP) {
      n = MAX_TICKS_PER_STEP;
      backlogMs = 0;
    } else {
      backlogMs -= n * tickMs;
    }
    if (n > 0) {
      const t0 = performance.now();
      sim.tick(n);
      const perTick = (performance.now() - t0) / n;
      sendRoadsIfChanged(sim);
      sendFrame(sim, perTick);
    }
  }
  setTimeout(step, 4);
}

self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const msg = e.data;
  switch (msg.type) {
    case 'init':
      await init();
      sim = new Sim(msg.seed, msg.cols, msg.rows);
      tickMs = sim.dt() * 1000;
      sim.spawn_agents(msg.agents);
      sendRoadsIfChanged(sim);
      sendZonesIfChanged(sim);
      sendLotsIfChanged(sim);
      last = performance.now();
      step();
      break;
    case 'setSpeed':
      speed = msg.speed;
      break;
    case 'spawn':
      sim?.spawn_agents(msg.count);
      break;
    case 'buildRoad':
    case 'buildCurve': {
      const success = sim
        ? msg.type === 'buildCurve'
          ? sim.build_curve(msg.ax, msg.ay, msg.cx, msg.cy, msg.bx, msg.by, msg.class)
          : sim.build_road(msg.ax, msg.ay, msg.bx, msg.by, msg.class)
        : false;
      if (success && sim) {
        sendRoadsIfChanged(sim);
        sendLotsIfChanged(sim);
      }
      post({ type: 'buildResult', id: msg.id, success });
      break;
    }
    case 'deleteRoad':
      if (sim?.delete_road(msg.x, msg.y)) sendEdits(sim);
      break;
    case 'paintZone':
      if (sim?.paint_zone(msg.ax, msg.ay, msg.bx, msg.by, msg.zone)) sendEdits(sim);
      break;
    case 'fillBlock':
      if (sim?.fill_block(msg.x, msg.y, msg.zone)) sendEdits(sim);
      break;
    case 'previewBlock': {
      const outline = sim?.block_preview(msg.x, msg.y) ?? new Float32Array();
      post({ type: 'blockPreview', outline, revision: msg.revision }, [outline.buffer]);
      break;
    }
    case 'previewBulldoze': {
      const segment = sim?.bulldoze_preview(msg.x, msg.y) ?? new Float32Array();
      post({ type: 'bulldozePreview', segment, revision: msg.revision }, [segment.buffer]);
      break;
    }
    case 'previewRoad':
    case 'previewCurve': {
      const stops = sim
        ? msg.type === 'previewCurve'
          ? sim.curve_preview(msg.ax, msg.ay, msg.cx, msg.cy, msg.bx, msg.by)
          : sim.road_preview(msg.ax, msg.ay, msg.bx, msg.by)
        : new Float32Array();
      post({ type: 'roadPreview', revision: msg.revision, stops }, [stops.buffer]);
      break;
    }
    case 'previewSnap': {
      const stops = sim?.snap_preview(msg.x, msg.y) ?? new Float32Array();
      post({ type: 'roadPreview', stops, revision: msg.revision }, [stops.buffer]);
      break;
    }
  }
};
