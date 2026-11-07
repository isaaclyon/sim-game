// Turns the sim's packed road and grid drafts into preview messages (ADR 0038, 0049). Worker
// side: only unpacks the WASM answers; the sim guides, plans and prices.
import type { Sim } from './pkg/sim_wasm';
import type { FromWorker, ToWorker } from './protocol';

type Answer = { msg: FromWorker; transfer: Transferable[] };

/** A draft request's points for the sim: `[x, y, snap]` records, then the snap reach in metres
 * as one trailing float when the request has one (ADR 0038). */
export function rawDraft(msg: { points: number[]; reach?: number }) {
  return new Float32Array(msg.reach === undefined ? msg.points : [...msg.points, msg.reach]);
}

/** Answers `previewRoad`: `[n, m, length, angle, k, r]`, guided points, guide lines, angle
 * marks, `r` floats of roads the draft would change to its class, then stops. */
export function roadPreview(
  sim: Sim | undefined,
  msg: Extract<ToWorker, { type: 'previewRoad' }>,
): Answer {
  const raw = rawDraft(msg);
  const d = sim?.road_draft(raw, msg.plan, msg.class) ?? new Float32Array(6);
  const [n, m, length, angle, k, r] = d;
  const at = 6 + 2 * n;
  const points = d.slice(6, at);
  const guides = d.slice(at, at + 4 * m);
  const angles = d.slice(at + 4 * m, at + 4 * m + 6 * k);
  const reclass = d.slice(at + 4 * m + 6 * k, at + 4 * m + 6 * k + r);
  const stops = d.slice(at + 4 * m + 6 * k + r);
  // What the guided road would charge (ADR 0036); nothing when no road is planned.
  const cost = msg.plan && stops.length > 0 ? (sim?.draft_cost(raw, msg.class) ?? null) : null;
  const affordable = cost !== null && !!sim?.can_afford(cost);
  return {
    msg: {
      type: 'roadPreview',
      ...{ revision: msg.revision, stops, points, guides, angles, length, angle },
      ...{ cost, affordable, reclass },
    },
    transfer: [stops.buffer, points.buffer, guides.buffer, angles.buffer, reclass.buffer],
  };
}

/** Answers `previewGrid`: `[n, m, length, angle, k, cols, rows, cost, r]`, guided points,
 * guide lines, angle marks, new legs, then left-out edges (`World::write_grid_draft`). */
export function gridPreview(
  sim: Sim | undefined,
  msg: Extract<ToWorker, { type: 'previewGrid' }>,
): Answer {
  const cursor = new Float32Array(msg.cursor ?? []);
  const d =
    sim?.grid_draft(rawDraft(msg), cursor, msg.cell, msg.class) ??
    new Float32Array([0, 0, 0, Number.NaN, 0, 0, 0, Number.NaN, 0]);
  const [n, m, length, angle, k, cols, rows, price, r] = d;
  let at = 9;
  const take = (count: number) => {
    at += count;
    return d.slice(at - count, at);
  };
  const points = take(2 * n);
  const guides = take(4 * m);
  const angles = take(6 * k);
  const roads = take(6 * r);
  const dropped = d.slice(at);
  const cost = Number.isNaN(price) ? null : price;
  const affordable = cost !== null && !!sim?.can_afford(cost);
  return {
    msg: {
      type: 'gridPreview',
      ...{ revision: msg.revision, points, guides, angles, length, angle },
      ...{ cols, rows, roads, dropped, cost, affordable },
    },
    transfer: [points.buffer, guides.buffer, angles.buffer, roads.buffer, dropped.buffer],
  };
}
