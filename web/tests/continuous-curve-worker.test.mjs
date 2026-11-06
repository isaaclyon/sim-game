import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

test('continuous curve previews and builds the same smooth continuation across the worker', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const first = [-1000, -1000, 0, -950, -1000, 0, -900, -950, 0];
  await w.send({ type: 'buildRoad', id: 1, points: first, class: 0, chain: true });
  assert.equal(w.messages.at(-1).success, true);
  assert.deepEqual(Array.from(w.messages.at(-1).control), [-950, -1000]);
  const points = [-900, -950, 8, -850, -920, 0, -950, -1000, 0];
  await w.send({ type: 'previewRoad', points, plan: true, class: 0, revision: 1 });
  const preview = w.messages.at(-1);
  assert.equal(preview.points.length, 6);
  assert.ok(preview.stops.length > 6);
  const [ax, ay, cx, cy, bx, by] = preview.points;
  assert.ok(Math.abs(cx - ax - (cy - ay)) < 0.001, 'continuation follows prior tangent');
  await w.send({ type: 'buildRoad', id: 2, points, class: 0 });
  assert.equal(w.messages.at(-1).success, true);
  assert.deepEqual([...w.messages.at(-1).end], [bx, by]);
  const backwards = [bx, by, 8, bx - 60, by - 60, 0, cx, cy, 0];
  await w.send({ type: 'previewRoad', points: backwards, plan: true, class: 0, revision: 2 });
  assert.equal(w.messages.at(-1).stops.length, 0);
  await w.send({ type: 'buildRoad', id: 3, points: backwards, class: 0 });
  assert.equal(w.messages.at(-1).success, false);
});

test('five pieces keep their sim-guided controls through the worker', async () => {
  const w = await worker(0, 3, undefined, false);
  w.ready();
  await w.init;
  let points = [-1000, -1000, 0, -950, -1000, 0, -900, -950, 0];
  const targets = [
    [-840, -880],
    [-790, -800],
    [-750, -710],
    [-720, -610],
  ];
  for (let id = 1; id <= 5; id++) {
    await w.send({ type: 'buildRoad', id, points, class: 0, chain: true });
    const result = w.messages.at(-1);
    assert.equal(result.success, true, `piece ${id}`);
    if (id < 5) points = [...result.end, 8, ...targets[id - 1], 0, ...result.control, 0];
  }
});
