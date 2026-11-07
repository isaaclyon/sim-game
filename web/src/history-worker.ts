// The worker's undo and redo messages (ADR 0051): it asks the sim and sends what the sim says.
import type { Sim } from './pkg/sim_wasm';
import type { FromWorker, HistoryCommand, HistoryStatus, StepKind } from './protocol';

type Post = (msg: FromWorker, transfer?: Transferable[]) => void;

/** What undo and redo would do now, for the frame. */
export function readHistory(s: Sim): HistoryStatus {
  const v = s.history_view();
  const out: HistoryStatus = {
    undo: (v.undo as StepKind | undefined) ?? null,
    redo: (v.redo as StepKind | undefined) ?? null,
    undoRefund: v.undo_refund ?? null,
    redoCost: v.redo_cost ?? null,
    redoAffordable: v.redo_affordable,
  };
  v.free();
  return out;
}

/** Handles an undo, redo or preview; true if it changed the city. */
export function historyCommand(
  s: Sim | undefined,
  msg: HistoryCommand,
  ready: boolean,
  post: Post,
): boolean {
  if (msg.type === 'previewHistory') {
    const data = s?.history_preview(msg.redo) ?? new Float32Array(2);
    const split = 2 + (data[1] ?? 0);
    const remove = data.slice(2, split);
    const restore = data.slice(split);
    const { redo, revision } = msg;
    post({ type: 'historyPreview', redo, ok: data[0] === 1, remove, restore, revision }, [
      remove.buffer,
      restore.buffer,
    ]);
    return false;
  }
  const ok = !!s && ready && (msg.type === 'undo' ? s.undo() : s.redo());
  post({ type: 'historyResult', id: msg.id, ok });
  return ok;
}
