// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

export type ToWorker =
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number }
  | { type: 'setSpeed'; speed: number }
  /** Explicit synthetic benchmark traffic, not normal play. */
  | { type: 'spawn'; count: number }
  | {
      type: 'buildRoad';
      id: number;
      ax: number;
      ay: number;
      bx: number;
      by: number;
      class: RoadClass;
    }
  | {
      type: 'buildCurve';
      id: number;
      ax: number;
      ay: number;
      cx: number;
      cy: number;
      bx: number;
      by: number;
      class: RoadClass;
    }
  | { type: 'deleteRoad'; x: number; y: number }
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
  /** Asks where a road would snap. Changes nothing; answered with `roadPreview`. */
  | { type: 'previewRoad'; revision: number; ax: number; ay: number; bx: number; by: number }
  | {
      type: 'previewCurve';
      revision: number;
      ax: number;
      ay: number;
      cx: number;
      cy: number;
      bx: number;
      by: number;
    }
  /** Asks where a road started here would start. Answered with a one-stop `roadPreview`. */
  | { type: 'previewSnap'; revision: number; x: number; y: number }
  /**
   * Selects the resident who owns car `agent` (an index into the `frame` agents snapshot) for
   * the citizen inspector, or clears the selection with null (ADR 0022). The worker keeps the
   * citizen, not the car slot. Answered with `citizen` at once, paused or not, then after every
   * frame and edit while selected. `selection` is echoed so replies to an older one are dropped.
   */
  | { type: 'inspect'; agent: number | null; selection: number };

/** Road classes, matching the sim's `RoadClass` (ADR 0017). Main roads carry long
 * trips; the sim lays alleys to reach deep lots. */
export const RoadClass = {
  Street: 0,
  Main: 1,
  Alley: 2,
} as const;
export type RoadClass = (typeof RoadClass)[keyof typeof RoadClass];

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
  /** The citizen's own car: an index into `frame` agents; never reassigned to anyone else. */
  agent: number | null;
  /** On a road now, so the car is drawn. */
  driving: boolean;
}

export interface Stats {
  tick: number;
  /** Day clock from the sim (`sim::clock`): 1-based day and seconds since midnight. */
  day: number;
  secondOfDay: number;
  /** Resident individuals, including people at home/work/shop. Not a vehicle count. */
  population: number;
  /** Residents without an assigned job, from the sim. A disconnected job still counts. */
  unemployed: number;
  /** Sim-computed `unemployed` as a percentage of `population`; 0 with no residents. */
  unemploymentPercent: number;
  /** Allocated vehicle slots, including dormant reservations and departed citizens' slots. */
  agents: number;
  /** Synthetic vehicles waiting between trips; not residents at home or dormant slots. */
  parked: number;
  awaitingPath: number;
  entering: number;
  driving: number;
  tripsCompleted: number;
  pathsFailed: number;
  despawnedStuck: number;
  /** Cars removed because a road on their remaining route was split or deleted. */
  despawnedByEdit: number;
}

export type FromWorker =
  /**
   * `segments` is `[ax, ay, bx, by, class]` per active rendered segment (many per curve), not
   * per road ID. `class` is a `RoadClass`.
   */
  | { type: 'roads'; segments: Float32Array; bounds: [number, number, number, number] }
  /**
   * `stops` is `[x, y, join]` per stop of the previewed road, in order; join is 0 = new end,
   * 1 = existing node, 2 = splits an existing road, -1 = interior curve sample (no marker).
   * Empty means refused. Answers `previewRoad` / `previewCurve`, or `previewSnap` with one stop.
   * `revision` echoes the construction stage so cancelled or previous-stage previews are discarded.
   */
  | { type: 'roadPreview'; stops: Float32Array; revision: number }
  /**
   * The whole zone grid: one `Zone` byte per cell, row-major from the origin. `layout` is
   * `[originX, originY, cellSize, cols, rows]`; cell (c, r) spans x from originX + c * cellSize.
   * Sent at start and whenever a paint changes a cell.
   */
  | { type: 'zones'; cells: Uint8Array; layout: [number, number, number, number, number] }
  /**
   * `rings` is `[n, x0, y0, …, x(n-1), y(n-1)]` per lot: its outline's point count, then its
   * points (ADR 0018). Sent at start and whenever lots change.
   */
  | { type: 'lots'; rings: Float32Array }
  /**
   * `[zone, height, roof, n, x0, y0, …]` per active building, packed in building ID order, not
   * indexed by ID. Height is metres; n is the footprint point count, counter-clockwise,
   * without a repeated closing point. Zone is the original use, not the repainted zone.
   * Roof is a stable palette index (0–3 for industry; 0 for other uses).
   * Growth is coalesced to at most four snapshots per real second. Start, edits and pause
   * flush immediately, so paused road edits never leave displaced buildings visible.
   */
  | { type: 'buildings'; data: Float32Array }
  /** `[x, y]` per outline sample of the block under the cursor, closed; empty outside blocks. */
  | { type: 'blockPreview'; outline: Float32Array; revision: number }
  | { type: 'buildResult'; id: number; success: boolean }
  /** Selected road's `[x, y]` centreline samples, or empty on a miss. Sim-owned hit testing. */
  | { type: 'bulldozePreview'; segment: Float32Array; revision: number }
  /**
   * `agents` is `[x, y, heading, speedRatio]` per agent id; speedRatio < 0 means hidden.
   * `signals` is `[x, y, heading, state]` per signalised approach, at its stop line; state is
   * 0 = red, 1 = green, 2 = protected-left green.
   */
  /** The inspected citizen, or null if the selected car has no resident. Answers `inspect`. */
  | { type: 'citizen'; selection: number; citizen: Citizen | null }
  | {
      type: 'frame';
      agents: Float32Array;
      signals: Float32Array;
      stats: Stats;
      tickMs: number;
    };
