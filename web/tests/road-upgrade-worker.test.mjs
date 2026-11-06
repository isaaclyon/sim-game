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

test('clicking a road previews its centreline and price, then changes its class in place (ADR 0050)', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  await w.send({ type: 'buildRoad', id: 1, points: [-1000, -1000, 0, -800, -1000, 0], class: 0 });
  assert.equal(w.messages.at(-1).success, true);
  const roads = () => w.messages.findLast((m) => m.type === 'roads');
  const segments = roads().segments.length;

  await w.send({ type: 'previewRoadClass', x: -900, y: -995, class: 0, revision: 1 });
  let preview = w.messages.at(-1);
  assert.equal(preview.type, 'roadClassPreview');
  assert.equal(preview.revision, 1);
  assert.equal(preview.segment.length, 0, 'already a street: nothing to change');
  assert.equal(preview.cost, null);

  await w.send({ type: 'previewRoadClass', x: -900, y: -995, class: 1, revision: 2 });
  preview = w.messages.at(-1);
  assert.deepEqual([...preview.segment], [-1000, -1000, -800, -1000], 'the road that would change');
  assert.equal(preview.cost, 3000, '$15 per metre over 200 m');
  assert.equal(preview.affordable, true);

  const balance = () => w.messages.findLast((m) => m.type === 'frame').stats.balance;
  const funds = balance();
  const before = w.messages.length;
  await w.send({ type: 'setRoadClass', x: -900, y: -995, class: 1 });
  const after = roads();
  assert.ok(w.messages.indexOf(after) >= before, 'the new roads were sent');
  assert.equal(after.segments.length, segments, 'the same road, in place');
  assert.equal(after.segments.at(-1), 1, 'drawn as a main road');
  assert.equal(balance(), funds - 3000);

  await w.send({ type: 'commandLog' });
  assert.match(w.messages.at(-1).text, /\n\d+ class -900 -995 main\n/);
});
