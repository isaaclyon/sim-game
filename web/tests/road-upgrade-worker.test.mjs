import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

test('drawing a main road over a street previews, charges and builds an upgrade (ADR 0048)', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const points = [-1000, -1000, 0, -800, -1000, 0];
  await w.send({ type: 'buildRoad', id: 1, points, class: 0 });
  assert.equal(w.messages.at(-1).success, true);

  await w.send({ type: 'previewRoad', points, plan: true, class: 0, revision: 1 });
  assert.equal(w.messages.at(-1).stops.length, 0, 'same class: nothing to build');
  await w.send({ type: 'previewRoad', points, plan: true, class: 1, revision: 2 });
  const preview = w.messages.at(-1);
  assert.ok(preview.stops.length > 0, 'main over a street plans an upgrade');
  assert.equal(preview.cost, 3000, '$15 per metre over 200 m');

  await w.send({ type: 'buildRoad', id: 2, points, class: 1 });
  assert.equal(w.messages.at(-1).success, true);
});
