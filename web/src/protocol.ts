// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

export type ToWorker =
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number }
  | { type: 'setSpeed'; speed: number }
  | { type: 'spawn'; count: number }
  | { type: 'buildRoad'; ax: number; ay: number; bx: number; by: number }
  | { type: 'deleteRoad'; x: number; y: number }
  | { type: 'previewBulldoze'; x: number; y: number; revision: number }
  /** Asks where a road would snap. Changes nothing; answered with `roadPreview`. */
  | { type: 'previewRoad'; ax: number; ay: number; bx: number; by: number; revision: number }
  /** Asks where a road started here would start. Answered with a one-stop `roadPreview`. */
  | { type: 'previewSnap'; x: number; y: number; revision: number };

export interface Stats {
  tick: number;
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
  /** `segments` is `[ax, ay, bx, by, speedLimit]` per active road, packed (not indexed by road ID). */
  | { type: 'roads'; segments: Float32Array; bounds: [number, number, number, number] }
  /**
   * `stops` is `[x, y, join]` per stop of the previewed road, in order; join is 0 = new end,
   * 1 = existing node, 2 = splits an existing road. Empty means the road would be refused.
   * Answers `previewRoad`, or `previewSnap` with exactly one stop.
   */
  | { type: 'roadPreview'; stops: Float32Array; revision: number }
  /** Selected segment `[ax, ay, bx, by]`, or empty on a miss. Sim-owned hit testing. */
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
