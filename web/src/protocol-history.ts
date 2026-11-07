// Undo and redo (ADR 0051): the step kinds, the frame's history status and the worker messages
// that carry them. Re-exported from `protocol.ts`.

/** What a step did, from the sim's `StepKind`: a road, curve, grid, class change or bulldoze,
 * or the plant, line, pump, tower or station placed. */
export type StepKind =
  | 'road'
  | 'curve'
  | 'grid'
  | 'class'
  | 'bulldoze'
  | 'coal'
  | 'solar'
  | 'wind'
  | 'line'
  | 'pump'
  | 'tower'
  | 'station';

/** What the undo and redo buttons show, from the sim. Read-only. */
export interface HistoryStatus {
  /** The step undo would take back; null with nothing to undo. */
  undo: StepKind | null;
  redo: StepKind | null;
  /** Dollars undo would give back; null with nothing to undo. */
  undoRefund: number | null;
  /** Dollars redo would charge; null with nothing to redo. */
  redoCost: number | null;
  /** False when the city cannot pay for the redo, which would be refused. */
  redoAffordable: boolean;
}

export type HistoryCommand =
  /** Takes back the newest undoable edit, refunding what it cost. Refused when nothing is
   * left to undo or what it would restore no longer fits. Answered with `historyResult`. */
  | { type: 'undo'; id: number }
  /** Makes the newest undone edit again, charging what it cost. Refused when it no longer fits
   * or the city cannot pay. Answered with `historyResult`. */
  | { type: 'redo'; id: number }
  /** Asks what undo (or `redo`) would change. Answered with `historyPreview`. */
  | { type: 'previewHistory'; redo: boolean; revision: number };

export type HistoryMessage =
  | { type: 'historyResult'; id: number; ok: boolean }
  /** `remove` and `restore` are `[n, x0, y0, …]` per record undo (or `redo`) would take away
   * and put back: a road's centreline samples, a plant's, pump's, tower's or station's closed
   * outline, or a line's two ends. `ok` false means it would be refused now. Changes nothing. */
  | {
      type: 'historyPreview';
      redo: boolean;
      ok: boolean;
      remove: Float32Array;
      restore: Float32Array;
      revision: number;
    };
