// Messages between the main thread and the sim worker. The worker owns the sim; the main
// thread only sends commands and draws snapshots.

export type ToWorker =
  | { type: 'init'; seed: number; cols: number; rows: number; agents: number }
  | { type: 'setSpeed'; speed: number }
  | { type: 'spawn'; count: number }
  | { type: 'buildRoad'; ax: number; ay: number; bx: number; by: number };

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
}

export type FromWorker =
  /** `segments` is `[ax, ay, bx, by, speedLimit]` per road. */
  | { type: 'roads'; segments: Float32Array; bounds: [number, number, number, number] }
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
