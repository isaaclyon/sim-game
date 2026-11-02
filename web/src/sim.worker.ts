// Runs the WASM sim on a fixed-timestep clock, off the main thread.

import init, { Sim } from './pkg/sim_wasm';
import type { Activity, Citizen, FromWorker, Place, Stats, ToWorker, Zone } from './protocol';

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
let buildingsVersion = -1;
let buildingsSentAt = -Infinity;
/** The inspector's selection: a citizen, kept even when its car slot is hidden. */
let inspected: { selection: number; citizen: number } | null = null;
/** Full meshes are expensive; faster simulation must not flood the main thread. */
const BUILDING_SNAPSHOT_INTERVAL_MS = 250;

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
  const rings = s.lot_outlines();
  post({ type: 'lots', rings }, [rings.buffer]);
}

function sendBuildingsIfChanged(s: Sim, force = false) {
  const version = s.buildings_version();
  if (version === buildingsVersion) return;
  const now = performance.now();
  if (!force && now - buildingsSentAt < BUILDING_SNAPSHOT_INTERVAL_MS) return;
  buildingsVersion = version;
  buildingsSentAt = now;
  const data = s.building_snapshot();
  post({ type: 'buildings', data }, [data.buffer]);
}

/** After an edit: zoning can add alleys, and road edits can recut lots (ADR 0016). */
function sendEdits(s: Sim) {
  sendRoadsIfChanged(s);
  sendZonesIfChanged(s);
  sendLotsIfChanged(s);
  sendBuildingsIfChanged(s, true);
  sendFrame(s); // Cars and signals also change while paused.
}

function readStats(s: Sim): Stats {
  const st = s.stats();
  const out: Stats = {
    tick: st.tick,
    day: st.day,
    secondOfDay: st.second_of_day,
    agents: st.agents,
    population: st.population,
    unemployed: st.unemployed,
    unemploymentPercent: st.unemployment_percent,
    demand: {
      residential: st.demand_residential,
      commercial: st.demand_commercial,
      industrial: st.demand_industrial,
    },
    parked: st.parked,
    awaitingPath: st.awaiting_path,
    entering: st.entering,
    driving: st.driving,
    walking: st.walking,
    tripsCompleted: st.trips_completed,
    walksCompleted: st.walks_completed,
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
  sendCitizen(s);
}

function place(building: number | undefined, zone: number | undefined): Place | null {
  return building === undefined ? null : { building, zone: zone as Zone };
}

/** Details of the selected citizen, sent only while someone is selected. */
function sendCitizen(s: Sim) {
  if (!inspected) return;
  const v = s.citizen_info(inspected.citizen);
  let citizen: Citizen | null = null;
  if (v) {
    citizen = {
      id: inspected.citizen,
      name: v.name,
      resident: v.resident,
      home: place(v.home, v.home_zone),
      job: place(v.job, v.job_zone),
      activity: v.activity as Activity,
      location: place(v.location, v.location_zone),
      destination: place(v.destination, v.destination_zone),
      agent: v.agent ?? null,
      driving: v.driving,
      onFoot: v.on_foot,
    };
    v.free();
  }
  post({ type: 'citizen', selection: inspected.selection, citizen });
  if (!citizen) inspected = null;
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
      sendBuildingsIfChanged(sim);
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
      if (msg.agents > 0) sim.spawn_agents(msg.agents);
      sendRoadsIfChanged(sim);
      sendZonesIfChanged(sim);
      sendLotsIfChanged(sim);
      sendBuildingsIfChanged(sim, true);
      sendFrame(sim);
      last = performance.now();
      step();
      break;
    case 'setSpeed':
      speed = msg.speed;
      if (speed === 0 && sim) sendBuildingsIfChanged(sim, true);
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
      if (success && sim) sendEdits(sim);
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
    case 'inspect': {
      const citizen = msg.agent === null ? undefined : sim?.citizen_of_agent(msg.agent);
      if (citizen === undefined) {
        inspected = null;
        if (msg.agent !== null) post({ type: 'citizen', selection: msg.selection, citizen: null });
      } else if (sim) {
        inspected = { selection: msg.selection, citizen };
        sendCitizen(sim);
      }
      break;
    }
    case 'previewSnap': {
      const stops = sim?.snap_preview(msg.x, msg.y) ?? new Float32Array();
      post({ type: 'roadPreview', stops, revision: msg.revision }, [stops.buffer]);
      break;
    }
  }
};
