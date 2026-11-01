// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

export type ToWorker =
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number }
  | { type: 'setSpeed'; speed: number }
  | { type: 'spawn'; count: number }
  | { type: 'buildRoad'; id: number; ax: number; ay: number; bx: number; by: number }
  | {
      type: 'buildCurve';
      id: number;
      ax: number;
      ay: number;
      cx: number;
      cy: number;
      bx: number;
      by: number;
    }
  /** Asks where a road would snap. Changes nothing; answered with `roadPreview`. */
  | { type: 'previewRoad'; id: number; ax: number; ay: number; bx: number; by: number }
  | {
      type: 'previewCurve';
      id: number;
      ax: number;
      ay: number;
      cx: number;
      cy: number;
      bx: number;
      by: number;
    }
  /** Asks where a road started here would start. Answered with a one-stop `roadPreview`. */
  | { type: 'previewSnap'; id: number; x: number; y: number };

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
  /** Cars removed because a new road split a road on their route. */
  despawnedByEdit: number;
}

export type FromWorker =
  /** `segments` is `[ax, ay, bx, by, speedLimit]` per rendered segment (many per curve). */
  | { type: 'roads'; segments: Float32Array; bounds: [number, number, number, number] }
  /**
   * `stops` is `[x, y, join]` per stop of the previewed road, in order; join is 0 = new end,
   * 1 = existing node, 2 = splits an existing road, -1 = interior curve sample (no marker).
   * Empty means refused. Answers `previewRoad` / `previewCurve`, or `previewSnap` with one stop.
   * `id` echoes the construction stage so cancelled or previous-stage previews are discarded.
   */
  | { type: 'roadPreview'; id: number; stops: Float32Array }
  | { type: 'buildResult'; id: number; success: boolean }
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
