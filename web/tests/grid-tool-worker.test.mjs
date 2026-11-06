import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

const edge = [-1000, -1000, 0, -920, -1000, 0];

test('a grid previews its legs and price, then builds them all in one command (ADR 0049)', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });

  await w.send({ type: 'previewGrid', revision: 3, class: 0, cell: 80, points: edge });
  let preview = w.messages.at(-1);
  assert.equal(preview.type, 'gridPreview');
  assert.equal(preview.revision, 3);
  assert.deepEqual([...preview.points], [-1000, -1000, -920, -1000], 'the guided first edge');
  assert.deepEqual([preview.cols, preview.rows], [1, 1], 'without a cursor: one block');
  assert.equal(preview.roads.length, 4 * 6, 'four legs around one block');

  const cursor = [-840, -840];
  await w.send({ type: 'previewGrid', revision: 4, class: 0, cell: 80, points: edge, cursor });
  preview = w.messages.at(-1);
  assert.deepEqual([preview.cols, preview.rows], [2, 2]);
  assert.equal(preview.roads.length, 12 * 6, 'twelve one-block legs');
  assert.equal(preview.dropped.length, 0, 'nothing in the way');
  assert.ok(preview.cost > 0);
  assert.equal(preview.affordable, true);

  const balance = () => w.messages.findLast((m) => m.type === 'frame').stats.balance;
  const funds = balance();
  await w.send({ type: 'buildGrid', id: 7, class: 0, cell: 80, points: edge, cursor });
  assert.deepEqual(
    { ...w.messages.at(-1) },
    { type: 'buildResult', id: 7, success: true, end: null },
  );
  assert.ok(w.messages.some((m) => m.type === 'roads' && m.segments.length > 0));
  assert.equal(balance(), funds - preview.cost, 'charged what the preview quoted');

  await w.send({ type: 'previewGrid', revision: 5, class: 0, cell: 80, points: edge, cursor });
  preview = w.messages.at(-1);
  assert.equal(preview.roads.length, 0, 'the same grid again overlaps every road');
  assert.equal(preview.cost, null);
  await w.send({ type: 'buildGrid', id: 8, class: 0, cell: 80, points: edge, cursor });
  assert.equal(w.messages.at(-1).success, false);

  await w.send({ type: 'commandLog' });
  assert.match(w.messages.at(-1).text, /\n\d+ roadgrid -1000 -1000 1 0 80 2 2 street\n/);
});
