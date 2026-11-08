// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

import type { HistoryCommand, HistoryMessage, HistoryStatus } from './protocol-history.ts';
import type {
  FireStats,
  PlantKind,
  PowerStats,
  Prices,
  ServiceMessage,
  WaterKind,
  WaterStats,
} from './protocol-services.ts';

export * from './protocol-history.ts';
export * from './protocol-services.ts';

export type ToWorker =
  | HistoryCommand
  /**
   * `grid` starts on the generated `cols` × `rows` street grid (tests and benchmarks); false
   * starts on an empty map of that size, as the game does.
   */
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number; grid: boolean }
  | { type: 'setSpeed'; speed: number }
  /** Replace the single local save with the current sim state. */
  | { type: 'saveCity' }
  /** The UI must confirm before requesting a fresh city and replacing the save. */
  | { type: 'newCity' }
  /** Asks for the current city's save bytes to share; answered with `cityFile`. */
  | { type: 'exportCity' }
  /**
   * Replaces the current city with a shared copy (ADR 0035), opened paused and saved to the
   * local slot. The UI must confirm first when it would discard a city. Invalid bytes change
   * nothing.
   */
  | { type: 'openCity'; bytes: Uint8Array }
  /** Asks for the command log since a fresh start (ADR 0034); answered with `commandLog`. */
  | { type: 'commandLog' }
  /** Explicit synthetic benchmark traffic, not normal play. */
  | { type: 'spawn'; count: number }
  /** Builds the road through guided `points` and `reach`, as in `previewRoad`. `chain` requests
   * the sim-guided control of a successful curve in `buildResult`, for its next continuation. */
  | {
      type: 'buildRoad';
      id: number;
      points: number[];
      reach?: number;
      class: RoadClass;
      chain?: boolean;
    }
  /** Gives the road nearest the point, in bulldoze reach, `class` in place, as drawing over
   * the whole road would (ADR 0048, 0050). Refused for a road already of `class` or when the
   * city cannot pay. */
  | { type: 'setRoadClass'; x: number; y: number; class: RoadClass }
  /** Asks which road `setRoadClass` would change and for what. Answered with
   * `roadClassPreview`. Changes nothing. */
  | { type: 'previewRoadClass'; x: number; y: number; class: RoadClass; revision: number }
  /** Removes the fire station or plant under the point, else the nearest line, else the pump
   * or tower under the point (ADR 0052), else the building under the point (ADR 0045), else
   * ruins (ADR 0053), else the nearest road. */
  | { type: 'deleteRoad'; x: number; y: number }
  /** Places a power plant at the point: coal snaps beside the nearest road and faces it, others
   * centre on the point (ADR 0041, 0042). Refused, changing nothing, when it does not fit or
   * the city cannot pay (ADR 0052). */
  | { type: 'placePlant'; kind: PlantKind; x: number; y: number }
  /** Places a pump or water tower beside the nearest road, facing it (ADR 0052). Refused,
   * changing nothing, away from roads, over other pieces, or when the city cannot pay. */
  | { type: 'placeWater'; kind: WaterKind; x: number; y: number }
  /** Draws a transmission line; each end snaps to a line end, a plant's terminal, or the
   * sidewalk of a road in reach on the side facing the other end (ADR 0042). */
  | { type: 'buildLine'; ax: number; ay: number; bx: number; by: number }
  /** Places a fire station beside the nearest road, facing it (ADR 0053). Refused away from
   * roads, over roads, plants and stations, and when the city cannot pay. */
  | { type: 'placeStation'; x: number; y: number }
  /** Asks where a station asked for here would stand. Answered with `stationPreview`. */
  | { type: 'previewStation'; x: number; y: number; revision: number }
  /** Turns this city's fires on or off; off puts out every fire burning now (ADR 0053). */
  | { type: 'setFires'; on: boolean }
  /** Asks whether a plant fits here. Answered with `plantPreview`. */
  | { type: 'previewPlant'; kind: PlantKind; x: number; y: number; revision: number }
  /** Asks whether a pump or tower fits here. Answered with `waterPreview`. */
  | { type: 'previewWater'; kind: WaterKind; x: number; y: number; revision: number }
  /** Asks where a line would go. Answered with `linePreview`. */
  | { type: 'previewLine'; ax: number; ay: number; bx: number; by: number; revision: number }
  /**
   * Zones every cell whose centre is inside the rectangle with these corners (ADR 0014).
   * `zone` is a `Zone` value; `Zone.None` unzones.
   */
  | { type: 'paintZone'; ax: number; ay: number; bx: number; by: number; zone: Zone }
  /** Zones every cell of the block (land enclosed by roads) containing the point (ADR 0016). */
  | { type: 'fillBlock'; x: number; y: number; zone: Zone }
  /** Asks for the outline of the block containing the point. Answered with `blockPreview`. */
  | { type: 'previewBlock'; x: number; y: number; revision: number }
  | { type: 'previewBulldoze'; x: number; y: number; revision: number }
  /**
   * Asks how a road of `class` being drawn would be guided, built and charged (ADR 0036,
   * ADR 0038). `points` is `[x, y, snap]` per clicked point in drawing order, then the cursor:
   * one point asks where a road would start, two a straight road, three a curve (start,
   * control, end). For a continuation, the first point has `Snap.Continue`, followed by the
   * end and the previous curve's sim-guided control; the sim derives the new control. `snap`
   * otherwise selects drawing guides. `reach` is the snap reach in metres, about ten screen
   * pixels at the current zoom: guide lines, junctions and angle steps capture points within it,
   * never less than the sim's own reaches, which also apply when it is absent. `plan` false only
   * guides the points (a curve's control point). Roads the draft follows take `class` when built (ADR 0048), so a draft over roads
   * of the same class plans nothing. Changes nothing; answered with `roadPreview`.
   */
  | {
      type: 'previewRoad';
      revision: number;
      class: RoadClass;
      points: number[];
      reach?: number;
      plan: boolean;
    }
  /**
   * Asks what a grid of `class` roads would build (ADR 0049). `points` is the first edge's two
   * raw `[x, y, snap]` records, guided as a road's with `reach`; `cursor` is `[x, y]` where the area is
   * dragged to, rounded by the sim to whole blocks of `cell` metres (40 to 200), or absent
   * while the first edge is drawn. Changes nothing; answered with `gridPreview`.
   */
  | {
      type: 'previewGrid';
      revision: number;
      class: RoadClass;
      cell: number;
      points: number[];
      reach?: number;
      cursor?: [number, number];
    }
  /** Builds the grid `previewGrid` with the same fields shows, all or nothing; answered with
   * `buildResult` (`end` null). */
  | {
      type: 'buildGrid';
      id: number;
      class: RoadClass;
      cell: number;
      points: number[];
      reach?: number;
      cursor?: [number, number];
    }
  /**
   * Selects the resident who owns agent `agent` (a car or walker, an index into the `frame`
   * agents snapshot) for
   * the citizen inspector, or clears the selection with null (ADR 0022). The worker keeps the
   * citizen, not the car slot. Answered with `citizen` at once, paused or not, then after every
   * frame and edit while selected. `selection` is echoed so replies to an older one are dropped.
   */
  | { type: 'inspect'; agent: number | null; selection: number }
  /** Selects a stable building ID, replacing any citizen selection. Same refresh and
   * selection-counter contract as inspect; null clears either kind of selection. */
  | { type: 'inspectBuilding'; building: number | null; selection: number }
  /** A `frame`'s agents the page no longer draws, transferred back for the worker to refill. */
  | { type: 'spareAgents'; agents: Float32Array };

/** Road classes, matching the sim's `RoadClass` (ADR 0017). Main roads carry long
 * trips; the sim lays alleys to reach deep lots. */
export const RoadClass = {
  Street: 0,
  Main: 1,
  Alley: 2,
} as const;
export type RoadClass = (typeof RoadClass)[keyof typeof RoadClass];

/** Drawing guide flags, matching `sim::guide` (ADR 0038). Joining roads is always on. */
export const Snap = {
  Angle: 1,
  Guides: 2,
  Length: 4,
  Continue: 8,
} as const;

/** Zone cell values, matching the sim's `Zone` (ADR 0014). Density is part of the zone. */
export const Zone = {
  None: 0,
  ResidentialLow: 1,
  ResidentialHigh: 2,
  CommercialLow: 3,
  CommercialHigh: 4,
  IndustrialLow: 5,
  IndustrialHigh: 6,
} as const;
export type Zone = (typeof Zone)[keyof typeof Zone];

/** What a citizen is doing or travelling to do, matching the sim's `Activity`. */
export const Activity = {
  Home: 0,
  Work: 1,
  Shop: 2,
} as const;
export type Activity = (typeof Activity)[keyof typeof Activity];

/** Floats per agent in the `frame` agents snapshot. */
export const AGENT_STRIDE = 5;
/** The `kind` field of an agent snapshot entry (ADR 0026, 0053). */
export const AgentKind = {
  Car: 0,
  Walker: 1,
  FireTruck: 2,
} as const;
export type AgentKind = (typeof AgentKind)[keyof typeof AgentKind];

/** A building, named by its ID and original use. */
export interface Place {
  building: number;
  zone: Zone;
}

/**
 * Sim-owned details of one citizen. Places are null when absent, or when their building was
 * removed; all are null once the citizen has left town (`resident` false).
 */
export interface Citizen {
  id: number;
  /** Derived from the world seed and IDs; a shared surname means a shared home, not kinship. */
  name: string;
  resident: boolean;
  home: Place | null;
  job: Place | null;
  /** The current activity, or the one the current trip is for. */
  activity: Activity;
  /** Where the citizen is, or where the current trip started. */
  location: Place | null;
  /** Where the current trip goes; null while staying put. */
  destination: Place | null;
  /** The citizen's own travel slot: an index into `frame` agents; never reassigned. */
  agent: number | null;
  /** On a road now, so the car is drawn. */
  driving: boolean;
  /** The current trip is on foot (ADR 0026); the walker is drawn once it sets off. */
  onFoot: boolean;
  /** Age in years from the sim; a child until 18 (ADR 0054). */
  age: number;
  /** A child, who lives at home with their household. */
  child: boolean;
  /** An adult who came of age in town rather than arriving grown up (ADR 0054). */
  grewUp: boolean;
  /** A grown child still living with their family, looking for a home of their own. */
  wantsHome: boolean;
}

/** Sim-owned counts, not physical attendance. Original use persists during closure. */
export interface Building extends Place {
  /** Resident places for homes, total jobs for workplaces. */
  capacity: number;
  /** People living here or employees assigned here, including those currently elsewhere. */
  occupied: number;
  closing: boolean;
  /** Starter power or its grid covers it (ADR 0041); if not, its vacancies stay empty. */
  powered: boolean;
  /** Has water, or is low density and needs none (ADR 0052); if not, its vacancies stay
   * empty and, after a day, its people slowly leave. */
  watered: boolean;
}

export interface Demand {
  residential: number;
  commercial: number;
  industrial: number;
}

export interface Stats {
  tick: number;
  /** Day clock from the sim (`sim::clock`): 1-based day and seconds since midnight. */
  day: number;
  secondOfDay: number;
  /** Resident individuals, children included, at home/work/shop. Not a vehicle count. */
  population: number;
  /** Residents who are children (ADR 0054); the rest are adults. */
  children: number;
  /** Adults without an assigned job, from the sim. A disconnected job still counts. */
  unemployed: number;
  /** Sim-computed `unemployed` as a percentage of adults; 0 with no adults. */
  unemploymentPercent: number;
  /** R/C/I demand from the sim, in people (ADR 0025). Growth also needs demand above its
   * land-value threshold, available power and, when dense, water; negative demand is a
   * surplus. */
  demand: Demand;
  power: PowerStats;
  water: WaterStats;
  /** Allocated travel slots, including dormant reservations and departed citizens' slots. */
  agents: number;
  /** Synthetic vehicles waiting between trips; not residents at home or dormant slots. */
  parked: number;
  awaitingPath: number;
  entering: number;
  driving: number;
  /** Citizens on foot now. */
  walking: number;
  /** Includes walks. */
  tripsCompleted: number;
  /** Trips made on foot (ADR 0026). */
  walksCompleted: number;
  pathsFailed: number;
  despawnedStuck: number;
  /** Cars removed because a road on their remaining route was split or deleted. */
  despawnedByEdit: number;
  /** The city's money in whole dollars, from the sim (ADR 0036); negative is debt. */
  balance: number;
  /** Taxes from residents and filled jobs, dollars per clock hour. */
  taxes: number;
  /** Flat grant every city receives, dollars per clock hour (ADR 0036). */
  grant: number;
  /** Road and fire station upkeep, dollars per clock hour (ADR 0036, 0053). */
  upkeep: number;
  /** Upkeep of plants, pumps and towers, dollars per clock hour (ADR 0052). */
  utilities: number;
  fire: FireStats;
}

/** Persistence metadata only. A commit acknowledges the captured revision, not later edits. */
export interface CityStatus {
  type: 'cityStatus';
  ready: boolean;
  busy: boolean;
  dirty: boolean;
  savedAt: number | null;
  error: string | null;
}

export type FromWorker =
  | CityStatus
  /** The current city's opaque save bytes, or null with a readable error. */
  | { type: 'cityFile'; bytes: Uint8Array | null; error: string | null }
  /** Command log text (ADR 0034), or null for a city that was loaded, not started fresh. */
  | { type: 'commandLog'; text: string | null }
  /** Sent before all geometry and the first frame of a newly installed world. Clears
   * interpolation and transient selections/previews. Restored cities always resume paused.
   * `prices` lists what plants, pumps, towers and stations cost (ADR 0036). */
  | { type: 'cityReset'; speed: number; prices: Prices }
  /**
   * `segments` is `[ax, ay, bx, by, class]` per active rendered segment (many per curve), not
   * per road ID. `class` is a `RoadClass`.
   */
  | { type: 'roads'; segments: Float32Array; bounds: [number, number, number, number] }
  /**
   * `stops` is `[x, y, join]` per stop of the previewed road, in order; join is 0 = new end,
   * 1 = existing node, 2 = splits an existing road, -1 = interior curve sample (no marker).
   * Empty means refused, or not asked for (`plan` false). One stop answers a one-point request.
   * `points` is the guided `[x, y]` per point; a continuation returns start, derived control and
   * end, or no points if refused. `guides` is `[x0, y0, x1, y1]` per
   * guide line used for the last point, from the line's origin to the point. `length` is metres
   * along the road so far; `angle` is degrees from the road its start joins, NaN for none.
   * `angles` is `[x, y, rx, ry, dx, dy]` per angle mark: a junction where the road's start (or,
   * when planned, its end) joins a road, the unit direction along the road measured from, and
   * along the new road, both away from the junction.
   * `revision` echoes the construction stage so cancelled or previous-stage previews are discarded.
   * `cost` is what building the guided road would charge in dollars, null when no road is
   * planned (one point, a control point, or refused);
   * `affordable` is false when the city cannot pay it, and building would be refused (ADR 0036).
   * `reclass` is `[n, x0, y0, …]` per existing road the build would change to the drawn class
   * (ADR 0048): its centreline sample count, then the samples; empty when none would change.
   */
  | {
      type: 'roadPreview';
      stops: Float32Array;
      points: Float32Array;
      guides: Float32Array;
      angles: Float32Array;
      length: number;
      angle: number;
      revision: number;
      cost: number | null;
      affordable: boolean;
      reclass: Float32Array;
    }
  /**
   * What a grid would build (ADR 0049). `points`, `guides`, `length`, `angle` and `angles`
   * describe the guided first edge as in `roadPreview`. `cols` and `rows` are the signed block
   * counts (0 when nothing is planned); `roads` is `[ax, ay, aJoin, bx, by, bJoin]` per new
   * leg, joins as in `roadPreview`'s stops; `dropped` is `[ax, ay, bx, by]` per lattice edge
   * left out. `cost` is dollars, null when nothing would be built; `affordable` false means the
   * build would be refused for money.
   */
  | {
      type: 'gridPreview';
      revision: number;
      points: Float32Array;
      guides: Float32Array;
      angles: Float32Array;
      length: number;
      angle: number;
      cols: number;
      rows: number;
      roads: Float32Array;
      dropped: Float32Array;
      cost: number | null;
      affordable: boolean;
    }
  /**
   * The whole zone grid: one `Zone` byte per cell, row-major from the origin. `layout` is
   * `[originX, originY, cellSize, cols, rows]`; cell (c, r) spans x from originX + c * cellSize.
   * Sent at start and whenever a paint changes a cell.
   */
  | { type: 'zones'; cells: Uint8Array; layout: [number, number, number, number, number] }
  /**
   * The whole pollution grid (ADR 0037): `[air, ground, noise, landValue]` bytes per cell,
   * row-major from the origin, 0 = none (for land value: worthless) to 255 = full. `layout` is
   * as for `zones`; cells are 4×4 zone cells. Sent at start, after edits and pauses, and at
   * most once a real second while it changes.
   */
  | { type: 'pollution'; cells: Uint8Array; layout: [number, number, number, number, number] }
  /**
   * `rings` is `[n, x0, y0, …, x(n-1), y(n-1)]` per lot: its outline's point count, then its
   * points (ADR 0018). Sent at start and whenever lots change.
   */
  | { type: 'lots'; rings: Float32Array }
  /**
   * `[zone, height, roof, front, n, x0, y0, …]` per active building, packed in building ID
   * order, not indexed by ID. Height is metres; n is the footprint point count,
   * counter-clockwise, without a repeated closing point. Zone is the original use, not the
   * repainted zone. Roof is a stable palette index (0–3 for industry; 0 for other uses).
   * Front is the footprint edge, from point `front` to the next, facing the lot's road (ADR 0027).
   * Closing buildings stay until they empty out (ADR 0023). `ids` contains the stable ID
   * for each packed record in this same message, never a snapshot index.
   * Growth and closure are coalesced to at most four snapshots per real second. Start, edits and pause
   * flush immediately, so paused road edits never leave displaced buildings visible.
   */
  | { type: 'buildings'; data: Float32Array; ids: Uint32Array }
  | ServiceMessage
  | HistoryMessage
  /** `[x, y]` per outline sample of the block under the cursor, closed; empty outside blocks. */
  | { type: 'blockPreview'; outline: Float32Array; revision: number }
  /** `end` is the built road's guided end, where a chained road starts; null when refused. */
  | {
      type: 'buildResult';
      id: number;
      success: boolean;
      end: [number, number] | null;
      control?: [number, number];
    }
  /** Selected road's `[x, y]` centreline samples, or empty on a miss. Sim-owned hit testing.
   * A station's, plant's, pump's, tower's, building's or ruins' target is its closed outline
   * and a line's its two ends. */
  | { type: 'bulldozePreview'; segment: Float32Array; revision: number }
  /** `[x, y]` centreline samples of the road `setRoadClass` would change, as for
   * `bulldozePreview`; empty when it would change nothing (no road in reach, or already that
   * class). `cost` is dollars, null when empty; `affordable` false means it would be refused
   * for money (ADR 0050). */
  | {
      type: 'roadClassPreview';
      segment: Float32Array;
      cost: number | null;
      affordable: boolean;
      revision: number;
    }
  /** The inspected citizen, or null if the selected agent has no resident. Answers `inspect`. */
  | { type: 'citizen'; selection: number; citizen: Citizen | null }
  /** Selected building, or null on a miss or after demolition. Answers inspectBuilding. */
  | { type: 'building'; selection: number; building: Building | null }
  /**
   * `agents` is `[x, y, heading, speedRatio, kind]` per agent id (`AGENT_STRIDE`); speedRatio
   * < 0 means hidden; kind is an `AgentKind` (ADR 0026, 0053).
   * `signals` is `[x, y, heading, state]` per signalised approach, at its stop line; state is
   * 0 = red, 1 = green, 2 = protected-left green. `history` is what undo and redo would do
   * (ADR 0051). Sent after ticks at most about 60 times a real second, and after edits. The
   * agents view may sit in a larger reused buffer; return it with `spareAgents` once replaced.
   */
  | {
      type: 'frame';
      agents: Float32Array;
      signals: Float32Array;
      stats: Stats;
      history: HistoryStatus;
      tickMs: number;
    };
