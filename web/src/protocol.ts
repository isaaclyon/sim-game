// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

export type ToWorker =
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number }
  | { type: 'setSpeed'; speed: number }
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
  | { type: 'previewSnap'; revision: number; x: number; y: number };

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

export interface Stats {
  tick: number;
  /** Day clock from the sim (`sim::clock`): 1-based day and seconds since midnight. */
  day: number;
  secondOfDay: number;
  agents: number;
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
  | {
      type: 'frame';
      agents: Float32Array;
      signals: Float32Array;
      stats: Stats;
      tickMs: number;
    };
