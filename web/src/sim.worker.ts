// Runs the WASM sim on a fixed-timestep clock, off the main thread.

import { CitySession } from './city-session';
import { gridPreview, roadPreview } from './drafts';
import init, { type Sim } from './pkg/sim_wasm';
import type {
  Activity,
  Building,
  Citizen,
  FromWorker,
  Place,
  Stats,
  ToWorker,
  Zone,
} from './protocol';

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
let powerVersion = -1;
let powerSentAt = -Infinity;
let waterVersion = -1;
let waterSentAt = -Infinity;
let pollutionVersion = -1;
let pollutionSentAt = -Infinity;
/** One inspector subject. Citizens persist even while their travel slot is hidden. */
let inspected: { selection: number; kind: 'citizen' | 'building'; id: number } | null = null;
/** Full meshes are expensive; faster simulation must not flood the main thread. */
const BUILDING_SNAPSHOT_INTERVAL_MS = 250;
/** Pollution changes slowly (every 10 sim seconds) and its grid can be large. */
const POLLUTION_SNAPSHOT_INTERVAL_MS = 1000;

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
  const ids = s.building_ids();
  post({ type: 'buildings', data, ids }, [data.buffer, ids.buffer]);
}

/** Power changes with growth too, so it is coalesced like buildings (ADR 0041). */
function sendPowerIfChanged(s: Sim, force = false) {
  const version = s.power_version();
  if (version === powerVersion) return;
  const now = performance.now();
  if (!force && now - powerSentAt < BUILDING_SNAPSHOT_INTERVAL_MS) return;
  powerVersion = version;
  powerSentAt = now;
  const plants = s.plant_snapshot();
  const lines = s.line_snapshot();
  const buildings = s.power_buildings();
  post({ type: 'power', plants, lines, buildings }, [
    plants.buffer,
    lines.buffer,
    buildings.buffer,
  ]);
}

/** Water changes with growth and tower levels, so it is coalesced too (ADR 0052). */
function sendWaterIfChanged(s: Sim, force = false) {
  const version = s.water_version();
  if (version === waterVersion) return;
  const now = performance.now();
  if (!force && now - waterSentAt < BUILDING_SNAPSHOT_INTERVAL_MS) return;
  waterVersion = version;
  waterSentAt = now;
  const pieces = s.water_pieces();
  const buildings = s.water_buildings();
  post({ type: 'water', pieces, buildings }, [pieces.buffer, buildings.buffer]);
}

function sendPollutionIfChanged(s: Sim, force = false) {
  const version = s.pollution_version();
  if (version === pollutionVersion) return;
  const now = performance.now();
  if (!force && now - pollutionSentAt < POLLUTION_SNAPSHOT_INTERVAL_MS) return;
  pollutionVersion = version;
  pollutionSentAt = now;
  const cells = s.pollution_cells();
  const [x0, y0, cell, cols, rows] = s.pollution_layout();
  post({ type: 'pollution', cells, layout: [x0, y0, cell, cols, rows] }, [cells.buffer]);
}

/** After an edit: zoning can add alleys, and road edits can recut lots (ADR 0016). */
function sendEdits(s: Sim) {
  sendRoadsIfChanged(s);
  sendZonesIfChanged(s);
  sendLotsIfChanged(s);
  sendBuildingsIfChanged(s, true);
  sendPowerIfChanged(s, true);
  sendWaterIfChanged(s, true);
  sendPollutionIfChanged(s, true);
  sendFrame(s); // Cars and signals also change while paused.
}

const cities = new CitySession(post, (next, initialSpeed) => {
  sim = next;
  tickMs = next.dt() * 1000;
  speed = initialSpeed;
  backlogMs = 0;
  last = performance.now();
  roadsVersion =
    zonesVersion =
    lotsVersion =
    buildingsVersion =
    powerVersion =
    waterVersion =
    pollutionVersion =
      -1;
  buildingsSentAt = powerSentAt = waterSentAt = pollutionSentAt = -Infinity;
  inspected = null;
  post({ type: 'cityReset', speed });
  sendEdits(next);
});
let started = false;

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
    power: {
      supply: st.power_supply,
      starter: st.power_starter,
      used: st.power_used,
      demand: st.power_demand,
      unpowered: st.power_unpowered,
      waiting: st.power_waiting,
    },
    water: {
      supply: st.water_supply,
      used: st.water_used,
      demand: st.water_demand,
      stored: st.water_stored,
      capacity: st.water_capacity,
      dry: st.water_dry,
      waiting: st.water_waiting,
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
    balance: st.balance,
    taxes: st.taxes,
    grant: st.grant,
    upkeep: st.upkeep,
    utilities: st.utilities,
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
  sendBuilding(s);
}

function place(building: number | undefined, zone: number | undefined): Place | null {
  return building === undefined ? null : { building, zone: zone as Zone };
}

/** Details of the selected citizen, sent only while someone is selected. */
function sendCitizen(s: Sim) {
  if (inspected?.kind !== 'citizen') return;
  const v = s.citizen_info(inspected.id);
  let citizen: Citizen | null = null;
  if (v) {
    citizen = {
      id: inspected.id,
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

/** One building only, using the same live query contract as citizen inspection. */
function sendBuilding(s: Sim) {
  if (inspected?.kind !== 'building') return;
  const v = s.building_info(inspected.id);
  const building: Building | null = v
    ? {
        building: v.building,
        zone: v.zone as Zone,
        capacity: v.capacity,
        occupied: v.occupied,
        closing: v.closing,
        powered: v.powered,
        watered: v.watered,
      }
    : null;
  v?.free();
  post({ type: 'building', selection: inspected.selection, building });
  if (!building) inspected = null;
}

function step() {
  const now = performance.now();
  backlogMs += (now - last) * speed;
  last = now;

  if (sim && cities.ready) {
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
      cities.changed();
      const perTick = (performance.now() - t0) / n;
      sendRoadsIfChanged(sim);
      sendBuildingsIfChanged(sim);
      sendPowerIfChanged(sim);
      sendWaterIfChanged(sim);
      sendPollutionIfChanged(sim);
      sendFrame(sim, perTick);
    }
  }
  setTimeout(step, 4);
}

self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const msg = e.data;
  switch (msg.type) {
    case 'init':
      if (started) return;
      started = true;
      await init();
      await cities.start(msg);
      last = performance.now();
      step();
      break;
    case 'saveCity':
      await cities.save();
      break;
    case 'newCity':
      await cities.newCity();
      break;
    case 'openCity':
      await cities.open(msg.bytes);
      break;
    case 'exportCity': {
      let bytes: Uint8Array | null = null;
      let error: string | null = null;
      try {
        bytes = cities.bytes();
      } catch (e) {
        error = `Could not export the city: ${e instanceof Error ? e.message : String(e)}`;
      }
      post({ type: 'cityFile', bytes, error }, bytes ? [bytes.buffer] : []);
      break;
    }
    case 'commandLog':
      post({ type: 'commandLog', text: cities.commandLog() });
      break;
    case 'setSpeed':
      if (!cities.ready) break;
      speed = msg.speed;
      if (speed === 0 && sim) {
        sendBuildingsIfChanged(sim, true);
        sendPowerIfChanged(sim, true);
        sendWaterIfChanged(sim, true);
        sendPollutionIfChanged(sim, true);
      }
      break;
    case 'spawn':
      if (cities.ready && sim?.spawn_agents(msg.count)) cities.changed();
      break;
    case 'buildRoad': {
      // Capture the sim-guided control before the build changes the network.
      const draft =
        msg.chain && sim && cities.ready
          ? sim.road_draft(new Float32Array(msg.points), false, msg.class)
          : null;
      const control: [number, number] | undefined =
        draft?.[0] === 3 ? [draft[8], draft[9]] : undefined;
      const built =
        sim && cities.ready ? sim.build_draft(new Float32Array(msg.points), msg.class) : null;
      const end: [number, number] | null = built?.length === 2 ? [built[0], built[1]] : null;
      if (end && sim) {
        cities.changed();
        sendEdits(sim);
      }
      post({
        type: 'buildResult',
        id: msg.id,
        success: end !== null,
        end,
        ...(end && control ? { control } : {}),
      });
      break;
    }
    case 'setRoadClass':
    case 'deleteRoad':
      if (
        cities.ready &&
        sim &&
        (msg.type === 'setRoadClass'
          ? sim.set_road_class(msg.x, msg.y, msg.class)
          : sim.delete_road(msg.x, msg.y))
      ) {
        cities.changed();
        sendEdits(sim);
      }
      break;
    case 'placePlant':
    case 'placeWater':
    case 'buildLine':
      if (
        cities.ready &&
        sim &&
        (msg.type === 'placePlant'
          ? sim.place_plant(msg.kind, msg.x, msg.y)
          : msg.type === 'placeWater'
            ? sim.place_water(msg.kind, msg.x, msg.y)
            : sim.build_line(msg.ax, msg.ay, msg.bx, msg.by))
      ) {
        cities.changed();
        sendEdits(sim);
      }
      break;
    case 'previewPlant':
    case 'previewWater': {
      const plant = msg.type === 'previewPlant';
      const data =
        (plant
          ? sim?.plant_preview(msg.kind, msg.x, msg.y)
          : sim?.water_preview(msg.kind, msg.x, msg.y)) ?? new Float32Array();
      const outline = data.slice(1);
      const type = plant ? 'plantPreview' : 'waterPreview';
      post({ type, outline, ok: data[0] === 1, revision: msg.revision }, [outline.buffer]);
      break;
    }
    case 'previewLine': {
      const ends = sim?.line_preview(msg.ax, msg.ay, msg.bx, msg.by) ?? new Float32Array();
      post({ type: 'linePreview', ends, revision: msg.revision }, [ends.buffer]);
      break;
    }
    case 'paintZone':
      if (cities.ready && sim?.paint_zone(msg.ax, msg.ay, msg.bx, msg.by, msg.zone)) {
        cities.changed();
        sendEdits(sim);
      }
      break;
    case 'fillBlock':
      if (cities.ready && sim?.fill_block(msg.x, msg.y, msg.zone)) {
        cities.changed();
        sendEdits(sim);
      }
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
    case 'previewRoadClass': {
      const segment = sim?.class_preview(msg.x, msg.y, msg.class) ?? new Float32Array();
      const cost = segment.length > 0 ? (sim?.class_cost(msg.x, msg.y, msg.class) ?? null) : null;
      const affordable = cost !== null && !!sim?.can_afford(cost);
      post({ type: 'roadClassPreview', segment, cost, affordable, revision: msg.revision }, [
        segment.buffer,
      ]);
      break;
    }
    case 'previewRoad':
    case 'previewGrid': {
      const answer = msg.type === 'previewRoad' ? roadPreview(sim, msg) : gridPreview(sim, msg);
      post(answer.msg, answer.transfer);
      break;
    }
    case 'buildGrid': {
      const cursor = new Float32Array(msg.cursor ?? []);
      const points = new Float32Array(msg.points);
      const success =
        !!sim && cities.ready && sim.build_grid_draft(points, cursor, msg.cell, msg.class);
      if (success && sim) {
        cities.changed();
        sendEdits(sim);
      }
      post({ type: 'buildResult', id: msg.id, success, end: null });
      break;
    }
    case 'inspect': {
      const citizen = msg.agent === null ? undefined : sim?.citizen_of_agent(msg.agent);
      if (citizen === undefined) {
        inspected = null;
        if (msg.agent !== null) post({ type: 'citizen', selection: msg.selection, citizen: null });
      } else if (sim) {
        inspected = { selection: msg.selection, kind: 'citizen', id: citizen };
        sendCitizen(sim);
      }
      break;
    }
    case 'inspectBuilding': {
      inspected =
        msg.building === null
          ? null
          : {
              selection: msg.selection,
              kind: 'building',
              id: msg.building,
            };
      if (sim) sendBuilding(sim);
      break;
    }
  }
};
